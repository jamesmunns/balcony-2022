#![no_main]
#![no_std]

#![allow(unused_imports)]

use balcony::{self as _, GlobalRollingTimer}; // global logger + panicking-behavior + memory layout

use maxwell::Demon;
use rand_chacha::{ChaCha8Rng, rand_core::{SeedableRng, RngCore}};
use smart_leds::{SmartLedsWrite, RGB8, gamma};
use stm32g0xx_hal as hal;
use hal::{stm32, rcc::{Config, PllConfig, Prescaler, RccExt}, gpio::GpioExt, spi::{Spi, NoSck, NoMiso}, time::RateExtU32, analog::adc::AdcExt};
use ws2812_spi::MODE;
use ws2812_spi::prerendered::Ws2812;
use smart_leds::colors;
use groundhog::RollingTimer;

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

use hal::analog::adc::{OversamplingRatio, Precision, SampleTime, VTemp, VBat};

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
    let _gpiob = board.GPIOB.split(&mut rcc);

    let mut adc = board.ADC.constrain(&mut rcc);
    adc.set_sample_time(SampleTime::T_80);
    adc.set_precision(Precision::B_12);
    adc.set_oversampling_ratio(OversamplingRatio::X_16);
    adc.set_oversampling_shift(16);
    adc.oversampling_enable(true);

    GlobalRollingTimer::init(board.TIM2);
    let timer = GlobalRollingTimer::new();
    let start = timer.get_ticks();

    while timer.micros_since(start) <= 20 { }
    adc.calibrate();

    let mut vbat = VBat::new();
    let mut vtemp = VTemp::new();
    vbat.enable(&mut adc);
    vtemp.enable(&mut adc);

    // We need 8 words of entropy
    let mut seed = [0u8; 32];
    let half = seed.len() / 2;
    let (seed_tmp, seed_bat) = seed.split_at_mut(half);

    let seed_start = timer.get_ticks();

    seed_tmp.chunks_exact_mut(4).for_each(|w| {
        let mut maxwell = Demon::default();
        let entropy = loop {
            let samp = adc.read_voltage(&mut vtemp).unwrap_or(0) as u32;
            match maxwell.take_sample(samp) {
                Ok(ent) => break ent,
                Err(_) => {},
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
                Err(maxwell::Error::NeedMoreSamples) => {},
            }
        };
        w.copy_from_slice(&entropy);

    });

    let elapsed = timer.micros_since(seed_start);

    let mut rng = ChaCha8Rng::from_seed(seed);
    let val = rng.next_u64();

    defmt::println!("random: {:016X}", val);
    defmt::println!("took: {}us", elapsed);

    // let mut smartled_spi = Spi::spi1(
    //         board.SPI1,
    //         (NoSck, NoMiso, gpiob.pb5),
    //         MODE,
    //         3_800_000.Hz(),
    //         &mut rcc,
    //     );
    // smartled_spi.half_duplex_enable(true);
    // smartled_spi.half_duplex_output_enable(true);
    // let mut buf = [0u8; (12 * 100) + 40];

    // let mut smartled = Ws2812::new(smartled_spi, &mut buf);



    // let mut forever = [
    //     colors::RED,
    //     colors::ORANGE,
    //     colors::YELLOW,
    //     colors::GREEN,
    //     colors::BLUE,
    //     colors::INDIGO,
    //     colors::VIOLET,
    //     colors::WHITE,
    //     colors::BLACK,
    // ].into_iter().cycle();

    // let mut chunky = [colors::BLACK; 100];
    // let mut idx = 0;

    // for _ in 0..1000 {
    //     chunky[idx] = forever.next()?;
    //     idx += 1;
    //     if idx >= chunky.len() {
    //         idx = 0;
    //     }

    //     let (left, right) = chunky.split_at(idx);

    //     let start = timer.get_ticks();

    //     // grb

    //     let seq = right.iter().chain(left.iter()).copied().map(|c| {
    //         RGB8 { r: c.g, g: c.r, b: c.b }
    //     });


    //     smartled.write(gamma(seq)).ok()?;
    //     while timer.millis_since(start) < 500 { }
    // }


    Some(())
}
