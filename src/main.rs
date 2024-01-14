#![no_std]
#![no_main]

use ch32v_rt::entry;
use ch32v00x_hal as hal;
use hal::gpio::PinState;
use hal::prelude::*;
use hal::pac::Peripherals;
use hal::println;
// use panic_halt as _;
use fugit::HertzU32 as Hertz;

use core::panic::PanicInfo;
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    println!("PANIC: {}", _info);
    loop {}
}

pub mod ds1302 {
    use core::panic;

    use ch32v00x_hal as hal;
    use hal::println;
    use embedded_hal::digital::v2::InputPin;
    use embedded_hal::digital::v2::OutputPin;

    pub const WRITE: u8 = 0x80;
    pub const READ: u8 = 0x81;
    pub const BURST: u8 = 0x40;
    pub const CLOCK_BURST: u8 = 0xBE;
    pub const RAM_BURST: u8 = 0xFE;
    pub const CLOCK: u8 = 0xC0;
    pub const RAM: u8 = 0xFE;

    pub trait Delay {
        fn delay_micro(&self, secs: u32);
        fn delay_nano(&self, secs: u32);
    }

    pub struct Ds1302<CE, SCLK, MISO, MOSI> {
        ce: CE,
        sclk: SCLK,
        miso: MISO,
        mosi: MOSI,
    }

    impl<CE, SCLK, MISO, MOSI> Ds1302<CE, SCLK, MISO, MOSI> {
        pub fn new(ce: CE, sclk: SCLK, miso: MISO, mosi: MOSI) -> Self {
            Self {
                ce,
                sclk,
                miso,
                mosi
            }
        }
    }

    impl<CE, SCLK, MISO, MOSI> Ds1302<CE, SCLK, MISO, MOSI>
    where CE: embedded_hal::digital::v2::OutputPin,
          SCLK: embedded_hal::digital::v2::OutputPin,
          MISO: embedded_hal::digital::v2::InputPin, 
          MOSI: embedded_hal::digital::v2::OutputPin,
    {
        // https://akizukidenshi.com/download/ds/maxim/ds1302.pdf
        
        pub fn write(&mut self, addr: u8, data: u8, delay: &impl Delay) {
            println!("write: addr: {:02X}, data: {:02X}", addr, data);
            let _result: Result<(), <SCLK as OutputPin>::Error> = self.sclk.set_low();
            let _result = self.ce.set_high();
            delay.delay_micro(4); // tCC = 4us for 2V
            // COMMAND BYTE
            // Figure 3. Address/Command Byte
            let mut command_byte = 0;
            for i in 0..8 {
                command_byte |= ((addr >> i) & 0b1) << (8 - i); //  The command byte is always input starting with the LSB (bit 0).
            }
            self.write_byte(command_byte, delay);
            self.write_byte(data, delay);
            delay.delay_nano(300); // tCCH = 240ns for 2V
            let _sresult = self.ce.set_low();
            delay.delay_micro(4); // tCWH = 4us for 2V
        }

        pub fn read(&mut self, addr: u8, delay: &impl Delay) -> Result<u8, <MISO as InputPin>::Error> {
            let _result = self.sclk.set_low();

            let _result = self.ce.set_high();
            delay.delay_micro(4); // tCC = 4us for 2V
            // COMMAND BYTE
            // Figure 3. Address/Command Byte
            let mut command_byte = 0;
            for i in 0..8 {
                command_byte |= ((addr >> i) & 0b1) << (8 - i); //  The command byte is always input starting with the LSB (bit 0).
            }
            self.write_byte(command_byte, delay);
            let data = self.read_byte(delay)?;
            delay.delay_nano(300); // tCCH = 240ns for 2V
            let _result = self.ce.set_low();
            delay.delay_micro(4); // tCWH = 4us for 2V
            println!("read:  addr: {:02X}, data: {:02X}", addr, data);
            Ok(data)
        }

        // 1バイトを読み込む
        fn read_byte(&mut self, delay: &impl Delay) -> Result<u8, <MISO as InputPin>::Error> {
            let mut data = 0;
            self.sclk.set_low();
            for i in 0..8 {
                let bit = self.read_bit(delay)?;
                data |= match bit { true => 1, false => 0, } << (8 - i);
            }
            Ok(data)
        }
        
        // 1ビットを読み込む
        fn read_bit(&mut self, delay: &impl Delay) -> Result<bool, <MISO as InputPin>::Error> {
            delay.delay_nano(300); // tCCZ = 280ns for 2V
            let _result: Result<(), <SCLK as OutputPin>::Error> = self.sclk.set_high();
            delay.delay_nano(1000); // tCH = 1000ns for 2V
            let bit = self.miso.is_high()?;
            let _result: Result<(), <SCLK as OutputPin>::Error> = self.sclk.set_low();
            delay.delay_nano(1000); // tCL = 1000ns for 2V
            Ok(bit)
        }

        // 1バイトを書き込む
        fn write_byte(&mut self, byte: u8, delay: &impl Delay) -> Result<(), <MOSI as OutputPin>::Error> {
            self.sclk.set_low();
            for i in 0..8 {
                self.write_bit((byte >> i) & 1 == 1, delay)?;
            }
            Ok(())
        }

        // 1ビットを書き込む
        fn write_bit(&mut self, bit: bool, delay: &impl Delay) -> Result<(), <MOSI as OutputPin>::Error> {
            let _ = self.mosi.set_state(bit.into());
            delay.delay_nano(250); // tDC = 200ns for 2V
            let _result: Result<(), <SCLK as OutputPin>::Error> = self.sclk.set_high();
            delay.delay_nano(1000); // tH = 1000ns for 2V
            let _result: Result<(), <SCLK as OutputPin>::Error> = self.sclk.set_low();
            delay.delay_nano(1000); // tCL = 1000ns for 2V
            Ok(())
        }
    }
}

struct Ch32Delay {
    sysclock: Hertz,
}

impl Ch32Delay {
    fn new(sysclock: Hertz) -> Self {
        Self {
            sysclock
        }
    }
    fn delay_milli(&self, secs: u32) {
        let cycles = self.sysclock.raw() / 1000 * secs / 2;
        unsafe {
            qingke::riscv::asm::delay(cycles);
        }
    }
}

impl ds1302::Delay for Ch32Delay {
    fn delay_micro(&self, secs: u32) {
        let cycles = self.sysclock.raw() / 1_000_000 * secs / 2;
        unsafe {
            qingke::riscv::asm::delay(cycles);
        }
    }
    fn delay_nano(&self, secs: u32) {
        let cycles = self.sysclock.raw() / 1_000_000_000 * secs / 2;
        unsafe {
            qingke::riscv::asm::delay(cycles);
        }
    }
}

#[entry]
fn main() -> ! {
    hal::debug::SDIPrint::enable();

    let pac = Peripherals::take().unwrap();

    let mut rcc = pac.RCC.constrain();
    let clocks = rcc.config.freeze();
    let delay = Ch32Delay::new(clocks.sysclk());

    let gpioa = pac.GPIOA.split(&mut rcc);
    let gpioc = pac.GPIOC.split(&mut rcc);
    let gpiod = pac.GPIOD.split(&mut rcc);

    // led
    let mut led_y = gpiod.pd6.into_push_pull_output();

    // ds1302
    let miso = gpioc.pc1.into_pull_up_input();
    let scl = gpioc.pc2.into_push_pull_output();
    let ce = gpioc.pc4.into_push_pull_output_in_state(PinState::Low);
    let mosi = gpioa.pa2.into_push_pull_output();

    let mut ds1302 = ds1302::Ds1302::new(ce, scl, miso, mosi);
    ds1302.write(0x80, 0b10000000, &delay);
    
    loop {
        delay.delay_milli(250);
        match ds1302.read(0x81, &delay) {
            Ok(data) => {
                if data & 0b0000_0001 == 0b0000_0001 {
                    led_y.set_high();
                } else {
                    led_y.set_low();
                }
            },
            Err(_) => {
                led_y.set_low();
            }
        }
        // led_y.toggle();
    }
}
