#![no_main]
#![no_std]

use balcony::{self as _, GlobalRollingTimer}; // global logger + panicking-behavior + memory layout

use groundhog::RollingTimer;
use hal::{
    analog::adc::{AdcExt, OversamplingRatio, Precision, SampleTime, VBat, VTemp},
    gpio::GpioExt,
    rcc::{Config, PllConfig, Prescaler, RccExt},
    spi::{NoMiso, NoSck, Spi},
    stm32,
    time::RateExtU32,
};
use maxwell::Demon;
use rand_chacha::{
    rand_core::{RngCore, SeedableRng},
    ChaCha8Rng,
};
use smart_leds::{
    colors,
    hsv::{hsv2rgb, Hsv},
};
use smart_leds::{gamma, SmartLedsWrite, RGB8};
use stm32g0xx_hal as hal;
use ws2812_spi::prerendered::Ws2812;
use ws2812_spi::MODE;

const HALF_LUT: [u8; 256] = [
    0, 3, 6, 9, 13, 16, 19, 22, 25, 28, 31, 34, 37, 41, 44, 47, 50, 53, 56, 59, 62, 65, 68, 71, 74,
    77, 80, 83, 86, 89, 92, 95, 98, 100, 103, 106, 109, 112, 115, 117, 120, 123, 126, 128, 131,
    134, 136, 139, 142, 144, 147, 149, 152, 154, 157, 159, 162, 164, 167, 169, 171, 174, 176, 178,
    180, 183, 185, 187, 189, 191, 193, 195, 197, 199, 201, 203, 205, 207, 208, 210, 212, 214, 215,
    217, 219, 220, 222, 223, 225, 226, 228, 229, 231, 232, 233, 234, 236, 237, 238, 239, 240, 241,
    242, 243, 244, 245, 246, 247, 247, 248, 249, 249, 250, 251, 251, 252, 252, 253, 253, 253, 254,
    254, 254, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 254, 254, 254, 253, 253, 253,
    252, 252, 251, 251, 250, 249, 249, 248, 247, 247, 246, 245, 244, 243, 242, 241, 240, 239, 238,
    237, 236, 234, 233, 232, 231, 229, 228, 226, 225, 223, 222, 220, 219, 217, 215, 214, 212, 210,
    208, 207, 205, 203, 201, 199, 197, 195, 193, 191, 189, 187, 185, 183, 180, 178, 176, 174, 171,
    169, 167, 164, 162, 159, 157, 154, 152, 149, 147, 144, 142, 139, 136, 134, 131, 128, 126, 123,
    120, 117, 115, 112, 109, 106, 103, 100, 98, 95, 92, 89, 86, 83, 80, 77, 74, 71, 68, 65, 62, 59,
    56, 53, 50, 47, 44, 41, 37, 34, 31, 28, 25, 22, 19, 16, 13, 9, 6, 3,
];

const SAT_LUT: [u8; 256] = [
    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    254, 254, 254, 254, 254, 254, 254, 254, 254, 254, 254, 254, 254, 254, 254, 254, 254, 254, 254,
    254, 254, 254, 254, 254, 253, 253, 253, 253, 253, 253, 253, 253, 253, 253, 253, 253, 253, 253,
    253, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 252, 251, 251, 251, 251, 251, 251,
    251, 251, 251, 250, 250, 250, 250, 250, 250, 250, 250, 249, 249, 249, 249, 249, 249, 249, 248,
    248, 248, 248, 248, 247, 247, 247, 247, 247, 246, 246, 246, 246, 246, 245, 245, 245, 245, 244,
    244, 244, 244, 243, 243, 243, 243, 242, 242, 242, 241, 241, 241, 240, 240, 240, 239, 239, 239,
    238, 238, 237, 237, 237, 236, 236, 235, 235, 234, 234, 233, 233, 232, 232, 231, 231, 230, 230,
    229, 229, 228, 227, 227, 226, 225, 225, 224, 223, 223, 222, 221, 220, 220, 219, 218, 217, 216,
    215, 215, 214, 213, 212, 211, 210, 209, 208, 207, 206, 204, 203, 202, 201, 200, 199, 197, 196,
    195, 193, 192, 191, 189, 188, 186, 185, 183, 182, 180, 178, 177, 175, 173, 171, 169, 167, 165,
    164, 161, 159, 157, 155, 153, 151, 148, 146, 144, 141, 139, 136, 133, 131, 128, 125, 122, 119,
    116, 113, 110, 107, 104, 100, 97, 94, 90, 86, 83, 79, 75, 71, 67, 63, 59, 54, 50, 45, 41, 36,
    31, 26, 21, 16, 11, 5,
];

#[cortex_m_rt::entry]
fn main() -> ! {
    defmt::println!("Hello, world!");

    if let Some(_) = imain() {
        defmt::println!("OK");
    } else {
        defmt::println!("ERR");
    }

    balcony::exit()
}

struct Node {
    hue: u8,
    sat: u8,
    phase: u16,
    rate: u16,
}

impl Node {
    const fn new() -> Self {
        Self {
            hue: 0,
            sat: 0,
            phase: 0,
            rate: 0,
        }
    }

    fn step(&mut self, rng: &mut ChaCha8Rng, new_hue: u8, new_sat: u8) -> RGB8 {
        if self.rate == 0 {
            self.hue = new_hue;
            self.sat = new_sat;
            self.phase = 0;
            self.rate = (rng.next_u32() & 0x3FF).max(128) as u16;
            colors::BLACK
        } else {
            match self.phase.checked_add(self.rate) {
                Some(val) => {
                    self.phase = val;
                    let idx_now = (self.phase >> 8) as u8;
                    let idx_nxt = idx_now.wrapping_add(1);

                    let base_val = HALF_LUT[idx_now as usize] as u16;
                    let next_val = HALF_LUT[idx_nxt as usize] as u16;

                    // Distance to next value
                    let off = self.phase & 0xFF; // 0..=255
                    let cur_weight = base_val.wrapping_mul(256u16.wrapping_sub(off));
                    let nxt_weight = next_val.wrapping_mul(off);
                    let ttl_weight = cur_weight.wrapping_add(nxt_weight);
                    let ttl_val = ttl_weight >> 8;
                    let ttl_val = ttl_val as u8;

                    hsv2rgb(Hsv {
                        hue: self.hue,
                        sat: self.sat,
                        val: ttl_val,
                    })
                }
                None => {
                    self.rate = 0;
                    colors::BLACK
                }
            }
        }
    }
}

fn imain() -> Option<()> {
    let board = stm32::Peripherals::take()?;
    let _core = stm32::CorePeripherals::take()?;

    // Configure clocks
    let config = Config::pll()
        .pll_cfg(PllConfig::with_hsi(1, 8, 2))
        .ahb_psc(Prescaler::NotDivided)
        .apb_psc(Prescaler::NotDivided);
    let mut rcc = board.RCC.freeze(config);

    let _gpioa = board.GPIOA.split(&mut rcc);
    let gpiob = board.GPIOB.split(&mut rcc);

    //////////
    // RNG
    //

    let mut adc = board.ADC.constrain(&mut rcc);
    adc.set_sample_time(SampleTime::T_80);
    adc.set_precision(Precision::B_12);
    adc.set_oversampling_ratio(OversamplingRatio::X_16);
    adc.set_oversampling_shift(16);
    adc.oversampling_enable(true);

    GlobalRollingTimer::init(board.TIM2);
    let timer = GlobalRollingTimer::new();
    let start = timer.get_ticks();

    while timer.micros_since(start) <= 20 {}
    adc.calibrate();

    let mut vbat = VBat::new();
    let mut vtemp = VTemp::new();
    vbat.enable(&mut adc);
    vtemp.enable(&mut adc);

    // We need 8 words of entropy
    let mut seed = [0u8; 32];
    let half = seed.len() / 2;
    let (seed_tmp, seed_bat) = seed.split_at_mut(half);

    seed_tmp.chunks_exact_mut(4).for_each(|w| {
        let mut maxwell = Demon::default();
        let entropy = loop {
            let samp = adc.read_voltage(&mut vtemp).unwrap_or(0) as u32;
            match maxwell.take_sample(samp) {
                Ok(ent) => break ent,
                Err(_) => {}
            }
        };
        w.copy_from_slice(&entropy);
    });
    seed_bat.chunks_exact_mut(4).for_each(|w| {
        let mut maxwell = Demon::default();
        let entropy = loop {
            let samp = adc.read_voltage(&mut vbat).unwrap_or(0) as u32;
            match maxwell.take_sample(samp) {
                Ok(ent) => break ent,
                Err(maxwell::Error::Timeout) => panic!(),
                Err(maxwell::Error::NeedMoreSamples) => {}
            }
        };
        w.copy_from_slice(&entropy);
    });

    let mut rng = ChaCha8Rng::from_seed(seed);

    /////////
    // smartleds

    let mut smartled_spi = Spi::spi1(
        board.SPI1,
        (NoSck, NoMiso, gpiob.pb5),
        MODE,
        3_800_000.Hz(),
        &mut rcc,
    );
    smartled_spi.half_duplex_enable(true);
    smartled_spi.half_duplex_output_enable(true);
    let mut buf = [0u8; (12 * 100) + 40];

    let mut smartled = Ws2812::new(smartled_spi, &mut buf);

    const NODE: Node = Node::new();
    let mut nodes = [NODE; 100];
    let mut storage = [colors::BLACK; 100];

    let mut hue: u8 = rng.next_u32() as u8;
    let mut sat: u8 = rng.next_u32() as u8;
    let mut last_color = timer.get_ticks();
    let mut last_draw = last_color;

    loop {
        if timer.ticks_since(last_color) >= 30_000_000 {
            hue = rng.next_u32() as u8;
            sat = SAT_LUT[rng.next_u32() as u8 as usize];
            last_color = timer.get_ticks();
        }

        nodes.iter_mut().zip(storage.iter_mut()).for_each(|(n, s)| {
            *s = n.step(&mut rng, hue, sat);
        });

        while timer.ticks_since(last_draw) < 31250 {}
        last_draw = timer.get_ticks();

        let seq = storage.iter().map(|c| RGB8 {
            r: c.g,
            g: c.r,
            b: c.b,
        });

        smartled.write(gamma(seq)).ok()?;
    }
}
