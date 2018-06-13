//! API for General Purpose I/O (GPIO)
//!
//! The entry point to this API is [`GPIO`]. It can be used to initialize the
//! peripheral, and is required by instances of [`Pin`] for GPIO functionality.
//! All [`Pin`] instances live in the [`swm`] module.
//!
//! The GPIO peripheral is described in the user manual, chapter 9.
//!
//! # Examples
//!
//! Initialize a GPIO pin and set its output to HIGH:
//!
//! ``` no_run
//! use lpc82x_hal::prelude::*;
//! use lpc82x_hal::Peripherals;
//!
//! let mut p = Peripherals::take().unwrap();
//!
//! let swm = p.swm.split();
//!
//! let pio0_12 = swm.pins.pio0_12
//!     .into_gpio_pin(&p.gpio)
//!     .into_output()
//!     .set_high();
//! ```
//!
//! Please refer to the [examples in the repository] for more example code.
//!
//! [`swm`]: ../swm/index.html
//! [examples in the repository]: https://github.com/braun-robotics/rust-lpc82x-hal/tree/master/examples


use embedded_hal::digital::{
    OutputPin,
    StatefulOutputPin,
};

use init_state::{
    self,
    InitState,
};
use raw;
use swm::{
    pin_state,
    Pin,
    PinTrait,
};
use syscon;



/// Represents the GPIO peripheral
///
/// Controls the GPIO peripheral. Can be used to enable, disable, or free the
/// peripheral. For GPIO-functionality directly related to pins, please refer
/// to [`Pin`].
///
/// Please refer to the [module documentation] for more information.
///
/// [module documentation]: index.html
pub struct GPIO<State: InitState = init_state::Enabled> {
    pub(crate) gpio  : raw::GPIO_PORT,
               _state: State,
}

impl GPIO<init_state::Enabled> {
    pub(crate) fn new(gpio: raw::GPIO_PORT) -> Self {
        GPIO {
            gpio  : gpio,
            _state: init_state::Enabled,
        }
    }
}

impl<'gpio> GPIO<init_state::Disabled> {
    /// Enable the GPIO peripheral
    ///
    /// Enables the clock and clears the peripheral reset for the GPIO
    /// peripheral.
    ///
    /// This method is only available, if `GPIO` is not already in the
    /// [`Enabled`] state. Code that attempts to call this method when the GPIO
    /// peripheral is already enabled will not compile.
    ///
    /// Consumes this instance of `GPIO` and returns another instance that has
    /// its `State` type parameter set to [`Enabled`].
    ///
    /// [`Enabled`]: ../init_state/struct.Enabled.html
    pub fn enable(mut self, syscon: &mut syscon::Handle)
        -> GPIO<init_state::Enabled>
    {
        syscon.enable_clock(&mut self.gpio);
        syscon.clear_reset(&mut self.gpio);

        GPIO {
            gpio  : self.gpio,
            _state: init_state::Enabled,
        }
    }
}

impl GPIO<init_state::Enabled> {
    /// Disable the GPIO peripheral
    ///
    /// This method is only available, if `GPIO` is not already in the
    /// [`Disabled`] state. Code that attempts to call this method when the GPIO
    /// peripheral is already disabled will not compile.
    ///
    /// Consumes this instance of `GPIO` and returns another instance that has
    /// its `State` type parameter set to [`Disabled`].
    ///
    /// [`Disabled`]: ../init_state/struct.Disabled.html
    pub fn disable(mut self, syscon: &mut syscon::Handle)
        -> GPIO<init_state::Disabled>
    {
        syscon.disable_clock(&mut self.gpio);

        GPIO {
            gpio  : self.gpio,
            _state: init_state::Disabled,
        }
    }
}

impl<State> GPIO<State> where State: InitState {
    /// Return the raw peripheral
    ///
    /// This method serves as an escape hatch from the HAL API. It returns the
    /// raw peripheral, allowing you to do whatever you want with it, without
    /// limitations imposed by the API.
    ///
    /// If you are using this method because a feature you need is missing from
    /// the HAL API, please [open an issue] or, if an issue for your feature
    /// request already exists, comment on the existing issue, so we can
    /// prioritize it accordingly.
    ///
    /// [open an issue]: https://github.com/braun-robotics/rust-lpc82x-hal/issues
    pub fn free(self) -> raw::GPIO_PORT {
        self.gpio
    }
}


impl<'gpio, T, D> Pin<T, pin_state::Gpio<'gpio, D>>
    where
        T: PinTrait,
        D: direction::NotOutput,
{
    /// Sets pin direction to output
    ///
    /// This method is only available, if the pin is in the GPIO state and the
    /// pin is not already in output mode, i.e. the pin direction is input or
    /// unknown. You can enter the GPIO state using [`into_gpio_pin`].
    ///
    /// Consumes the pin instance and returns a new instance that is in output
    /// mode, making the methods to set the output level available.
    ///
    /// **NOTE**: There are some doubts about whether this is the right API
    /// design. [Please leave your feedback](https://github.com/braun-robotics/rust-lpc82x-hal/issues/53),
    /// if you have anything to say about this.
    ///
    /// # Example
    ///
    /// ``` no_run
    /// # extern crate lpc82x;
    /// # extern crate lpc82x_hal;
    /// #
    /// # use lpc82x_hal::Peripherals;
    /// #
    /// # let mut p = Peripherals::take().unwrap();
    /// #
    /// # let mut syscon      = p.syscon.split();
    /// # let     swm         = p.swm.split();
    /// #
    /// # let pin = swm.pins.pio0_12
    /// #     .into_gpio_pin(&p.gpio);
    /// #
    /// use lpc82x_hal::prelude::*;
    ///
    /// // Assumes the pin is already in the GPIO state
    /// let mut pin = pin.into_output();
    ///
    /// // Output level can now be set
    /// pin.set_high();
    /// pin.set_low();
    /// ```
    ///
    /// [`into_gpio_pin`]: #method.into_gpio_pin
    pub fn into_output(self)
        -> Pin<T, pin_state::Gpio<'gpio, direction::Output>>
    {
        self.state.dirset0.write(|w|
            unsafe { w.dirsetp().bits(T::MASK) }
        );

        Pin {
            ty: self.ty,

            state: pin_state::Gpio {
                dirset0: self.state.dirset0,
                pin0   : self.state.pin0,
                set0   : self.state.set0,
                clr0   : self.state.clr0,

                _direction: direction::Output,
            }
        }
    }
}

impl<'gpio, T> OutputPin for Pin<T, pin_state::Gpio<'gpio, direction::Output>>
    where T: PinTrait
{
    /// Set the pin output to HIGH
    ///
    /// This method is only available, if two conditions are met:
    /// - The pin is in the GPIO state. Use [`into_gpio_pin`] to achieve this.
    /// - The pin direction is set to output. See [`into_output`].
    ///
    /// Unless both of these conditions are met, code trying to call this method
    /// will not compile.
    ///
    /// [`into_gpio_pin`]: #method.into_gpio_pin
    /// [`into_output`]: #method.into_output
    fn set_high(&mut self) {
        self.state.set0.write(|w|
            unsafe { w.setp().bits(T::MASK) }
        )
    }

    /// Set the pin output to LOW
    ///
    /// This method is only available, if two conditions are met:
    /// - The pin is in the GPIO state. Use [`into_gpio_pin`] to achieve this.
    /// - The pin direction is set to output. See [`into_output`].
    ///
    /// Unless both of these conditions are met, code trying to call this method
    /// will not compile.
    ///
    /// [`into_gpio_pin`]: #method.into_gpio_pin
    /// [`into_output`]: #method.into_output
    fn set_low(&mut self) {
        self.state.clr0.write(|w|
            unsafe { w.clrp().bits(T::MASK) }
        );
    }
}

impl<'gpio, T> StatefulOutputPin
    for Pin<T, pin_state::Gpio<'gpio, direction::Output>>
    where T: PinTrait
{
    /// Indicates whether the pin output is currently set to HIGH
    ///
    /// This method is only available, if two conditions are met:
    /// - The pin is in the GPIO state. Use [`into_gpio_pin`] to achieve this.
    /// - The pin direction is set to output. See [`into_output`].
    ///
    /// Unless both of these conditions are met, code trying to call this method
    /// will not compile.
    ///
    /// [`into_gpio_pin`]: #method.into_gpio_pin
    /// [`into_output`]: #method.into_output
    fn is_set_high(&self) -> bool {
        self.state.pin0.read().port().bits() & T::MASK == T::MASK
    }

    /// Indicates whether the pin output is currently set to LOW
    ///
    /// This method is only available, if two conditions are met:
    /// - The pin is in the GPIO state. Use [`into_gpio_pin`] to achieve this.
    /// - The pin direction is set to output. See [`into_output`].
    ///
    /// Unless both of these conditions are met, code trying to call this method
    /// will not compile.
    ///
    /// [`into_gpio_pin`]: #method.into_gpio_pin
    /// [`into_output`]: #method.into_output
    fn is_set_low(&self) -> bool {
        !self.state.pin0.read().port().bits() & T::MASK == T::MASK
    }
}


/// Contains types to indicate the direction of GPIO pins
///
/// Please refer to [`Pin`] for documentation on how these types are used.
pub mod direction {
    /// Implemented by types that indicate GPIO pin direction
    ///
    /// The [`Gpio`] type uses this trait as a bound for its type parameter.
    /// This is done for documentation purposes, to clearly show which types can
    /// be used for this parameter. Other than that, this trait should not be
    /// relevant to users of this crate.
    ///
    /// [`Gpio`]: ../../swm/pin_state/struct.Gpio.html
    pub trait Direction {}

    /// Marks a GPIO pin's direction as being unknown
    ///
    /// This type is used as a type parameter of [`Gpio`], which in turn is used
    /// as a type parameter of [`Pin`]. Please refer to the documentation of
    /// [`Pin`] to see how this type is used.
    ///
    /// As we can't know what happened to the hardware before the HAL was
    /// initialized, this is the initial state of GPIO pins.
    ///
    /// [`Gpio`]: ../../swm/pin_state/struct.Gpio.html
    /// [`Pin`]: ../../swm/struct.Pin.html
    pub struct Unknown;
    impl Direction for Unknown {}

    /// Marks a GPIO pin as being configured for input
    ///
    /// This type is used as a type parameter of [`Gpio`], which in turn is used
    /// as a type parameter of [`Pin`]. Please refer to the documentation of
    /// [`Pin`] to see how this type is used.
    ///
    /// [`Gpio`]: ../../swm/pin_state/struct.Gpio.html
    /// [`Pin`]: ../../swm/struct.Pin.html
    pub struct Input;
    impl Direction for Input {}

    /// Marks a GPIO pin as being configured for output
    ///
    /// This type is used as a type parameter of [`Gpio`], which in turn is used
    /// as a type parameter of [`Pin`]. Please refer to the documentation of
    /// [`Pin`] to see how this type is used.
    ///
    /// [`Gpio`]: ../../swm/pin_state/struct.Gpio.html
    /// [`Pin`]: ../../swm/struct.Pin.html
    pub struct Output;
    impl Direction for Output {}


    /// Marks a direction as not being output (i.e. being unknown or input)
    ///
    /// This is a helper trait used only to prevent some code duplication in
    /// [`Pin`] by allowing `impl` blocks to be defined precisely. It should not
    /// be relevant to users of this crate.
    ///
    /// [`Pin`]: ../../swm/struct.Pin.html
    pub trait NotOutput: Direction {}

    impl NotOutput for Unknown {}
    impl NotOutput for Input {}
}
