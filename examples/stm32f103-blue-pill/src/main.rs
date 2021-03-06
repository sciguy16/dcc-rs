// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! blinky timer using interrupts on TIM2, adapted from blinky_timer_irq.rs example from
//! stm32f1xx-hal
//!
//! This assumes that a LED is connected to pa5 (sck/d13) as is the case on most nucleo board.

#![no_main]
#![no_std]

const LOCO_ADDR: u8 = 2;

#[macro_use]
extern crate defmt; // logging macros

use defmt_rtt as _;
use panic_halt as _;

use stm32f1xx_hal as hal;

use crate::hal::{
    adc,
    gpio::{gpioa, Output, PushPull},
    pac::{interrupt, Interrupt, Peripherals, TIM2},
    prelude::*,
    timer::{CounterUs, Event},
};

use core::cell::RefCell;
use cortex_m::interrupt::Mutex;
use cortex_m_rt::entry;

use dcc_rs::{packets::*, DccInterruptHandler};

// A type definition for the GPIO pin to be used for our LED
type DccDirPin = gpioa::PA0<Output<PushPull>>;

// Make DCC thingy globally available
static G_DCC: Mutex<RefCell<Option<DccInterruptHandler<DccDirPin>>>> =
    Mutex::new(RefCell::new(None));

// Make timer interrupt registers globally available
static G_TIM: Mutex<RefCell<Option<CounterUs<TIM2>>>> =
    Mutex::new(RefCell::new(None));

// place for sending packets
static TX_BUFFER: Mutex<RefCell<Option<(SerialiseBuffer, usize)>>> =
    Mutex::new(RefCell::new(None));

#[interrupt]
fn TIM2() {
    static mut DCC: Option<DccInterruptHandler<DccDirPin>> = None;
    static mut TIM: Option<CounterUs<TIM2>> = None;

    let dcc = DCC.get_or_insert_with(|| {
        cortex_m::interrupt::free(|cs| {
            // Move LED pin here, leaving a None in its place
            G_DCC.borrow(cs).replace(None).unwrap()
        })
    });

    let tim = TIM.get_or_insert_with(|| {
        cortex_m::interrupt::free(|cs| {
            // Move LED pin here, leaving a None in its place
            G_TIM.borrow(cs).replace(None).unwrap()
        })
    });

    if let Some((new_data, len)) =
        cortex_m::interrupt::free(|cs| TX_BUFFER.borrow(cs).replace(None))
    {
        dcc.write(&new_data[..len]).unwrap();
    }

    if let Ok(new_delay) = dcc.tick() {
        tim.start(new_delay.micros()).unwrap();
    }

    let _ = tim.wait();
}

#[entry]
fn main() -> ! {
    info!("Start boot");

    let dp = Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();

    let rcc = dp.RCC.constrain();
    let mut flash = dp.FLASH.constrain();

    // Prepare the alternate function I/O registers
    //let mut afio = dp.AFIO.constrain();
    // let clocks = rcc.cfgr.freeze(&mut flash.acr);
    let clocks = rcc
        .cfgr
        .use_hse(8.MHz())
        .sysclk(48.MHz())
        //.pclk1(8.MHz())
        .freeze(&mut flash.acr);
    // info!("adc freq: {}", clocks.adcclk());

    let mut gpioa = dp.GPIOA.split();
    let mut gpiob = dp.GPIOB.split();
    let mut gpioc = dp.GPIOC.split();

    info!("a");
    let dcc_pin = gpioa.pa0.into_push_pull_output(&mut gpioa.crl);

    let mut dcc = DccInterruptHandler::new(dcc_pin);
    let pkt = SpeedAndDirection::builder()
        .address(10)
        .unwrap()
        .speed(14)
        .unwrap()
        .direction(Direction::Forward)
        .build();
    info!("a");
    let mut buffer = SerialiseBuffer::default();
    let len = pkt.serialise(&mut buffer).unwrap();
    dcc.write(buffer.get(0..len).unwrap()).unwrap();
    info!("a");

    // Move the DCC thingy into our global storage
    cortex_m::interrupt::free(|cs| *G_DCC.borrow(cs).borrow_mut() = Some(dcc));
    info!("a");

    // Set up a timer expiring after 1s
    let mut timer = dp.TIM2.counter_us(&clocks);
    // Generate an interrupt when the timer expires
    info!("a");
    timer.start(50000.micros()).unwrap();
    info!("a");
    timer.listen(Event::Update);
    info!("a");

    // Move the timer into our global storage
    cortex_m::interrupt::free(|cs| {
        *G_TIM.borrow(cs).borrow_mut() = Some(timer)
    });
    info!("a");

    //enable TIM2 interrupt
    // cortex_m::peripheral::NVIC::unpend(Interrupt::TIM2);
    unsafe {
        cortex_m::peripheral::NVIC::unmask(Interrupt::TIM2);
    }
    info!("init complete");

    // make a delay thing to send packets
    let mut delay = cp.SYST.delay(&clocks);

    // LED to show when power is on
    let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);

    // set up ADC
    let mut adc1 = adc::Adc::adc1(dp.ADC1, clocks);
    let mut ch0 = gpiob.pb0.into_analog(&mut gpiob.crl);

    loop {
        // read the control
        let control: u16 = adc1.read(&mut ch0).unwrap();

        // it's a 12-bit ADC, range is 0..=4095
        // speed is -16..=16
        // use the following ranges:
        // 0..1600: -16..0
        // 2495..=4095: 0..=16
        // 1601..2495: 0
        let (speed, direction) = match control {
            0..=1600 => (28 - control / 57, Direction::Backward),
            2495..=4095 => ((control - 2494) / 57, Direction::Forward),
            _ => (0, Direction::Forward),
        };
        info!(
            "control set to: {}, speed is {} {}",
            control, speed, direction
        );
        let speed = (speed & 0x1f) as u8;
        if speed == 0 {
            led.set_high();
        } else {
            led.set_low();
        }

        //info!("tx, addr = {}", addr);
        // pop a new chunk of data into the buffer
        let pkt = SpeedAndDirection::builder()
            .address(LOCO_ADDR)
            .unwrap()
            .speed(speed)
            .unwrap()
            .direction(direction)
            .build();
        let mut buffer = SerialiseBuffer::default();
        let len = pkt.serialise(&mut buffer).unwrap();
        cortex_m::interrupt::free(|cs| {
            *TX_BUFFER.borrow(cs).borrow_mut() = Some((buffer, len))
        });

        delay.delay_ms(15u16);
    }
}
