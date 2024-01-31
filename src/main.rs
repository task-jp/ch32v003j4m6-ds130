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
    use ch32v00x_hal as hal;
    use hal::gpio::Output;
    use hal::gpio::PushPull;
    use hal::println;
    use embedded_hal::digital::v2::InputPin;
    use embedded_hal::digital::v2::OutputPin;

    pub trait Delay {
        fn delay_micro(&self, secs: u32);
        fn delay_nano(&self, secs: u32);
    }

    #[derive(Debug)]
    pub enum ReadWriteError {
        ChipSelectError,
        ClockError,
        ReadError,
        WriteError,
    }

    pub struct Ds1302<SCLK, const P: char, const N: u8, CE> {
        sclk: SCLK,
        io: Option<ch32v00x_hal::gpio::Pin<P, N, Output<PushPull>>>,
        ce: CE,
    }

    impl<SCLK, const P: char, const N: u8, CE> Ds1302<SCLK, P, N, CE> {
        pub fn new(sclk: SCLK, io: ch32v00x_hal::gpio::Pin<P, N, Output<PushPull>>, ce: CE) -> Self {
            Self {
                sclk,
                io: Some(io),
                ce,
            }
        }
    }

    fn encode_bcd(bcd: u8) -> u8 {
        bcd % 10 + ((bcd / 10) << 4)
    }

    fn decode_bcd(bcd: u8) -> u8 {
        (bcd & 0b0000_1111) + ((bcd & 0b1111_0000) >> 4) * 10
    }

    impl<SCLK, const P: char, const N: u8, CE> Ds1302<SCLK, P, N, CE>
    where 
        SCLK: OutputPin,
        CE: OutputPin,
    {
        pub fn is_running(&mut self, delay: &impl Delay) -> Result<bool, ReadWriteError> {
            let seconds = self.read(0x81, delay)?;
            Ok(seconds & 0b1000_0000 == 0b0000_0000)
        }

        pub fn get_seconds(&mut self, delay: &impl Delay) -> Result<u8, ReadWriteError> {
            let seconds = self.read(0x81, delay)?;
            Ok(decode_bcd(seconds & 0b0111_1111))
        }
        pub fn get_minutes(&mut self, delay: &impl Delay) -> Result<u8, ReadWriteError> {
            let minutes = self.read(0x83, delay)?;
            Ok(decode_bcd(minutes & 0b0111_1111))
        }
        pub fn get_hour(&mut self, delay: &impl Delay) -> Result<u8, ReadWriteError> {
            let hour = self.read(0x85, delay)?;
            let is_24h = hour & 0b1000_0000 == 0b0000_0000;
            let is_pm = hour & 0b0010_0000 == 0b0010_0000;
            if is_24h {
                Ok(decode_bcd(hour & 0b0011_1111))
            } else if is_pm {
                Ok(decode_bcd(hour & 0b0001_1111) + 12)
            } else {
                Ok(decode_bcd(hour & 0b0001_1111))
            }
        }

        pub fn get_date(&mut self, delay: &impl Delay) -> Result<u8, ReadWriteError> {
            let date = self.read(0x87, delay)?;
            Ok(decode_bcd(date & 0b0011_1111))
        }

        pub fn get_month(&mut self, delay: &impl Delay) -> Result<u8, ReadWriteError> {
            let month = self.read(0x89, delay)?;
            Ok(decode_bcd(month & 0b0001_1111))
        }

        pub fn get_day(&mut self, delay: &impl Delay) -> Result<u8, ReadWriteError> {
            let day = self.read(0x8B, delay)?;
            Ok(decode_bcd(day & 0b0000_0111))
        }

        pub fn get_year(&mut self, delay: &impl Delay) -> Result<u8, ReadWriteError> {
            let year = self.read(0x8D, delay)?;
            Ok(decode_bcd(year))
        }

        pub fn set_running(&mut self, is_running: bool, delay: &impl Delay) -> Result<(), ReadWriteError> {
            let mut seconds = self.read(0x81, delay)?;
            if is_running {
                seconds &= 0b0111_1111;
            } else {
                seconds |= 0b1000_0000;
            }
            self.write(0x80, seconds, delay)
        }

        pub fn set_seconds(&mut self, seconds: u8, delay: &impl Delay) -> Result<(), ReadWriteError> {
            let mut seconds = encode_bcd(seconds);
            if !self.is_running(delay)? {
                seconds |= 0b1000_0000;
            }
            self.write(0x80, seconds, delay)
        }

        pub fn set_minutes(&mut self, minutes: u8, delay: &impl Delay) -> Result<(), ReadWriteError> {
            let minutes = encode_bcd(minutes);
            self.write(0x82, minutes, delay)
        }

        pub fn set_hour(&mut self, hour: u8, delay: &impl Delay) -> Result<(), ReadWriteError> {
            let hour = encode_bcd(hour);
            self.write(0x84, hour, delay)
        }

        pub fn set_date(&mut self, date: u8, delay: &impl Delay) -> Result<(), ReadWriteError> {
            let date = encode_bcd(date);
            self.write(0x86, date, delay)
        }

        pub fn set_month(&mut self, month: u8, delay: &impl Delay) -> Result<(), ReadWriteError> {
            let month = encode_bcd(month);
            self.write(0x88, month, delay)
        }

        pub fn set_day(&mut self, day: u8, delay: &impl Delay) -> Result<(), ReadWriteError> {
            let day = encode_bcd(day);
            self.write(0x8A, day, delay)
        }

        pub fn set_year(&mut self, year: u8, delay: &impl Delay) -> Result<(), ReadWriteError> {
            let year = encode_bcd(year);
            self.write(0x8C, year, delay)
        }

        // https://akizukidenshi.com/download/ds/maxim/ds1302.pdf
        
        fn write(&mut self, addr: u8, data: u8, delay: &impl Delay) -> Result<(), ReadWriteError> {
            self.sclk.set_low().map_err(|_| ReadWriteError::ClockError)?;
            self.ce.set_high().map_err(|_| ReadWriteError::ChipSelectError)?;
            delay.delay_micro(4); // tCC = 4us for 2V
            self.write_byte(addr, delay)?;
            self.write_byte(data, delay)?;
            delay.delay_nano(300); // tCCH = 240ns for 2V
            self.ce.set_low().map_err(|_| ReadWriteError::ChipSelectError)?;
            delay.delay_micro(4); // tCWH = 4us for 2V
            Ok(())
        }

        fn read(&mut self, addr: u8, delay: &impl Delay) -> Result<u8, ReadWriteError> {
            self.sclk.set_low().map_err(|_| ReadWriteError::ClockError)?;
            self.ce.set_high().map_err(|_| ReadWriteError::ChipSelectError)?;
            delay.delay_micro(4); // tCC = 4us for 2V
            self.write_byte(addr, delay)?;
            let data = self.read_byte(delay)?;
            delay.delay_nano(300); // tCCH = 240ns for 2V
            self.ce.set_low().map_err(|_| ReadWriteError::ChipSelectError)?;
            delay.delay_micro(4); // tCWH = 4us for 2V
            // println!("read:  addr: {:02X}({:08b} <=> {:08b}), data: {:02X}({:08b})", addr, addr, command_byte, data, data);
            Ok(data)
        }

        // 1バイトを読み込む
        fn read_byte(&mut self, delay: &impl Delay) -> Result<u8, ReadWriteError> {
            let mut data = 0;
            self.sclk.set_low().map_err(|_| ReadWriteError::ClockError)?;
            let pin = self.io.take().unwrap().into_pull_up_input();
            for i in 0..8 {
                let bit = self.read_bit(&pin, delay)?;
                data |= match bit { true => 1, false => 0, } << i;
            }
            self.io = Some(pin.into_push_pull_output());
            Ok(data)
        }
        
        // 1ビットを読み込む
        fn read_bit(&mut self, pin: &impl InputPin, delay: &impl Delay) -> Result<bool, ReadWriteError> {
            delay.delay_nano(300); // tCCZ = 280ns for 2V
            self.sclk.set_high().map_err(|_| ReadWriteError::ClockError)?;
            let bit = pin.is_high().map_err(|_| ReadWriteError::ReadError)?;
            delay.delay_nano(2000); // tCH = 1000ns for 2V
            self.sclk.set_low().map_err(|_| ReadWriteError::ClockError)?;
            delay.delay_nano(1700); // tCL = 1000ns for 2V
            Ok(bit)
        }

        // 1バイトを書き込む
        fn write_byte(&mut self, byte: u8, delay: &impl Delay) -> Result<(), ReadWriteError> {
            self.sclk.set_low().map_err(|_| ReadWriteError::ClockError)?;
            let mut pin = self.io.take().unwrap();
            for i in 0..8 {
                self.write_bit(&mut pin, ((byte >> i) & 1) == 1, delay)?;
            }
            self.io = Some(pin);
            Ok(())
        }

        // 1ビットを書き込む
        fn write_bit(&mut self, pin: &mut impl OutputPin, bit: bool, delay: &impl Delay) -> Result<(), ReadWriteError> {
            let _ = pin.set_state(bit.into());
            delay.delay_nano(350); // tDC = 200ns for 2V
            self.sclk.set_high().map_err(|_| ReadWriteError::ClockError)?;
            delay.delay_nano(2000); // tCH = 1000ns for 2V
            self.sclk.set_low().map_err(|_| ReadWriteError::ClockError)?;
            delay.delay_nano(1700); // tCL = 1000ns for 2V
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

    let mut green = gpioa.pa1.into_push_pull_output();
    let mut red = gpioa.pa2.into_push_pull_output();

    let sclk = gpioc.pc4.into_push_pull_output();
    let io = gpioc.pc2.into_push_pull_output();
    let ce = gpioc.pc1.into_push_pull_output_in_state(PinState::Low);
    let mut ds1302 = ds1302::Ds1302::new(sclk, io, ce);

    ds1302.set_hour(23, &delay).unwrap();
    ds1302.set_minutes(59, &delay).unwrap();
    ds1302.set_seconds(30, &delay).unwrap();
    ds1302.set_running(true, &delay).unwrap();
    
    let mut last_seconds = 0xffu8;
    loop {
        delay.delay_milli(10);
        let seconds = ds1302.get_seconds(&delay).unwrap();
        if seconds == last_seconds {
            continue;
        }
        last_seconds = seconds;
        let minutes = ds1302.get_minutes(&delay).unwrap();
        let hour = ds1302.get_hour(&delay).unwrap();

        if hour == 0 && minutes < 5 {
            green.set_high();
            red.set_low();
        } else {
            green.set_low();
            red.set_high();
        }

        println!("{:02}:{:02}:{:02}", hour, minutes, seconds);
        delay.delay_milli(900);
    }
}
