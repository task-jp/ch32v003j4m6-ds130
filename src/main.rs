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

    fn decode_bcd(bcd: u8) -> u8 {
        bcd & 0b0000_1111 + (bcd >> 4) * 10
    }

    fn reverse_bits(mut n: u8) -> u8 {
        n = (n >> 4) | (n << 4);
        n = ((n & 0xCC) >> 2) | ((n & 0x33) << 2);
        n = ((n & 0xAA) >> 1) | ((n & 0x55) << 1);
        n
    }

    impl<CE, SCLK, MISO, MOSI> Ds1302<CE, SCLK, MISO, MOSI>
    where CE: embedded_hal::digital::v2::OutputPin,
          SCLK: embedded_hal::digital::v2::OutputPin,
          MISO: embedded_hal::digital::v2::InputPin, 
          MOSI: embedded_hal::digital::v2::OutputPin,
    {
        pub fn get_seconds(&mut self, delay: &impl Delay) -> Result<u8, <MISO as InputPin>::Error> {
            let seconds = self.read(0x81, delay)?;
            Ok(decode_bcd(seconds & 0b0111_1111))
        }
        pub fn get_minutes(&mut self, delay: &impl Delay) -> Result<u8, <MISO as InputPin>::Error> {
            let minutes = self.read(0x83, delay)?;
            Ok(decode_bcd(minutes & 0b0111_1111))
        }
        pub fn get_hour(&mut self, delay: &impl Delay) -> Result<u8, <MISO as InputPin>::Error> {
            let hour = self.read(0x85, delay)?;
            let is_24h = hour & 0b1000_0000 == 0b1000_0000;
            let is_pm = hour & 0b0010_0000 == 0b0010_0000;
            let hour = decode_bcd(hour & 0b0001_1111);
            if !is_24h || !is_pm {
                Ok(hour)
            } else {
                Ok(hour + 12)
            }
        }

        pub fn get_date(&mut self, delay: &impl Delay) -> Result<u8, <MISO as InputPin>::Error> {
            let date = self.read(0x87, delay)?;
            Ok(decode_bcd(date & 0b0011_1111))
        }

        pub fn get_month(&mut self, delay: &impl Delay) -> Result<u8, <MISO as InputPin>::Error> {
            let month = self.read(0x89, delay)?;
            Ok(decode_bcd(month & 0b0001_1111))
        }

        pub fn get_day(&mut self, delay: &impl Delay) -> Result<u8, <MISO as InputPin>::Error> {
            let day = self.read(0x8B, delay)?;
            Ok(decode_bcd(day & 0b0000_0111))
        }

        pub fn get_year(&mut self, delay: &impl Delay) -> Result<u8, <MISO as InputPin>::Error> {
            let year = self.read(0x8D, delay)?;
            Ok(decode_bcd(year))
        }

        pub fn set_seconds(&mut self, seconds: u8, delay: &impl Delay) {
            let seconds = reverse_bits(seconds);
            self.write(0x80, seconds, delay);
        }

        pub fn set_minutes(&mut self, minutes: u8, delay: &impl Delay) {
            let minutes = reverse_bits(minutes);
            self.write(0x82, minutes, delay);
        }

        pub fn set_hour(&mut self, hour: u8, delay: &impl Delay) {
            let hour = reverse_bits(hour);
            self.write(0x84, hour, delay);
        }

        pub fn set_date(&mut self, date: u8, delay: &impl Delay) {
            let date = reverse_bits(date);
            self.write(0x86, date, delay);
        }

        pub fn set_month(&mut self, month: u8, delay: &impl Delay) {
            let month = reverse_bits(month);
            self.write(0x88, month, delay);
        }

        pub fn set_year(&mut self, year: u8, delay: &impl Delay) {
            let year = reverse_bits(year);
            self.write(0x8C, year, delay);
        }

        // https://akizukidenshi.com/download/ds/maxim/ds1302.pdf
        
        pub fn write(&mut self, addr: u8, data: u8, delay: &impl Delay) {
            let _result: Result<(), <SCLK as OutputPin>::Error> = self.sclk.set_low();
            let _result = self.ce.set_high();
            delay.delay_micro(4); // tCC = 4us for 2V
            // COMMAND BYTE
            // Figure 3. Address/Command Byte
            let command_byte = reverse_bits(addr);
            // println!("write: addr: {:02X}({:08b} <=> {:08b}), data: {:02X}", addr, addr, command_byte, data);
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
            let command_byte = reverse_bits(addr);
            self.write_byte(command_byte, delay);
            let data = self.read_byte(delay)?;
            delay.delay_nano(300); // tCCH = 240ns for 2V
            let _result = self.ce.set_low();
            delay.delay_micro(4); // tCWH = 4us for 2V
            // println!("read:  addr: {:02X}({:08b} <=> {:08b}), data: {:02X}({:08b})", addr, addr, command_byte, data, data);
            Ok(data)
        }

        // 1バイトを読み込む
        fn read_byte(&mut self, delay: &impl Delay) -> Result<u8, <MISO as InputPin>::Error> {
            let mut data = 0;
            self.sclk.set_low();
            for i in 0..8 {
                let bit = self.read_bit(delay)?;
                data |= match bit { true => 1, false => 0, } << i;
            }
            Ok(data)
        }
        
        // 1ビットを読み込む
        fn read_bit(&mut self, delay: &impl Delay) -> Result<bool, <MISO as InputPin>::Error> {
            delay.delay_nano(2500); // tCCZ = 280ns for 2V
            let _result: Result<(), <SCLK as OutputPin>::Error> = self.sclk.set_high();
            let bit = self.miso.is_high()?;
            delay.delay_nano(10000); // tCH = 1000ns for 2V
            let _result: Result<(), <SCLK as OutputPin>::Error> = self.sclk.set_low();
            delay.delay_nano(10000); // tCL = 1000ns for 2V
            Ok(bit)
        }

        // 1バイトを書き込む
        fn write_byte(&mut self, byte: u8, delay: &impl Delay) -> Result<(), <MOSI as OutputPin>::Error> {
            self.sclk.set_low();
            for i in 0..8 {
                // println!("write_bit: {}: {:0X}", i, ((byte >> (7 - i)) & 1));
                self.write_bit(((byte >> (7 - i)) & 1) == 1, delay)?;
            }
            Ok(())
        }

        // 1ビットを書き込む
        fn write_bit(&mut self, bit: bool, delay: &impl Delay) -> Result<(), <MOSI as OutputPin>::Error> {
            let _ = self.mosi.set_state(bit.into());
            delay.delay_nano(2500); // tDC = 200ns for 2V
            let _result: Result<(), <SCLK as OutputPin>::Error> = self.sclk.set_high();
            delay.delay_nano(10000); // tCH = 1000ns for 2V
            let _result: Result<(), <SCLK as OutputPin>::Error> = self.sclk.set_low();
            delay.delay_nano(10000); // tCL = 1000ns for 2V
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
        let cycles = self.sysclock.raw() / 1_000_000 * secs * 1000;
        unsafe {
            qingke::riscv::asm::delay(cycles);
        }
    }
    fn delay_nano(&self, secs: u32) {
        let cycles = self.sysclock.raw() as u64/ 1_000_000_000u64 * secs as u64 * 1000u64;
        unsafe {
            qingke::riscv::asm::delay(cycles as u32);
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
    let miso = gpioc.pc2.into_floating_input();
    let scl = gpioc.pc4.into_push_pull_output();
    let ce = gpioc.pc1.into_push_pull_output_in_state(PinState::Low);
    let mosi = gpioa.pa2.into_push_pull_output();

    let mut ds1302 = ds1302::Ds1302::new(ce, scl, miso, mosi);
    ds1302.set_year(24, &delay);
    delay.delay_milli(500);
    ds1302.set_month(1, &delay);
    delay.delay_milli(500);
    ds1302.set_date(30, &delay);
    delay.delay_milli(500);
    ds1302.set_hour(15, &delay);
    delay.delay_milli(500);
    ds1302.write(0x80, 0b10000000, &delay);
    
    loop {
        delay.delay_milli(500);
        let year = ds1302.get_year(&delay).unwrap();
        println!("20{:02}", year);
        // let month = ds1302.get_month(&delay).unwrap();
        // let date= ds1302.get_date(&delay).unwrap();
        // let week_date= ds1302.get_day(&delay).unwrap();
        // let hour = ds1302.get_hour(&delay).unwrap();
        // let minutes = ds1302.get_minutes(&delay).unwrap();
        // let seconds = ds1302.get_seconds(&delay).unwrap();
        // println!("20{:02}/{:02}/{:02}({}) {:02}:{:02}:{:02}", year, month, date, ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][week_date as usize], hour, minutes, seconds);
    }
}
