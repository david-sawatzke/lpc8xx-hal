macro_rules! flags {
    (
        $(
            $bit_pos:expr,
            $access:ident,
            $flag_or_interrupt:ident,
            $name:ident,
            $description:expr;
        )*
    ) => {
        /// Used to query the state of USART flags
        ///
        /// See `USART::is_flag_set`.
        pub enum Flag {
            $(
                #[doc = $description]
                $name,
            )*
        }

        flags!(@interrupts, () $($flag_or_interrupt, $name, $description;)*);
    };

    (@reset, ro, $usart:expr, $bit_pos:expr) => {};
    (@reset, w1, $usart:expr, $bit_pos:expr) => {
        // Sound, as long as the flags specified in the macro match the
        // hardware.
        $usart.stat.write(|w| unsafe { w.bits(0x1 << $bit_pos) });
    };

    // Here's a bit of a trick to work around the fact that macros must always
    // evaluate to complete items, expressions, etc. A struct field is not a
    // complete thing in that sense, so a macro can't generate one. It needs to
    // generate the whole struct, which is what the following rules do.
    //
    // This variant gets called if the beginning of the input is only a flag. It
    // Ignores the flag and passes the rest of the input on.
    (@interrupts,
        ($($output:tt)*)
        flag, $name:ident, $description:expr;
        $($input:tt)*
    ) => {
        flags!(@interrupts, ($($output)*) $($input)*);
    };
    // This variant gets called, if the beginning of the input if both flag and
    // interrupt. It adds a field for the interrupt to the output and passes the
    // rest of the input on.
    (@interrupts,
        ($($output:tt)*)
        both, $name:ident, $description:expr;
        $($input:tt)*
    ) => {
        flags!(@interrupts,
            (
                $($output)*
                #[doc = $description]
                pub $name: bool,
            )
            $($input)*
        );
    };
    // This variant gets called, if there is no more input to parse. If
    // generates the final struct from the output that has built up so far.
    (@interrupts,
        ($($output:tt)*)
    ) => {
        /// Used to enable or disable USART interrupts
        ///
        /// See `USART::enable_interrupts` and `USART::disable_interrupts`.
        #[allow(non_snake_case)]
        pub struct Interrupts {
            $($output)*
        }
    };
}

flags!(
     0, ro, both, RXRDY,      "Receiver ready";
     1, ro, flag, RXIDLE,     "Receiver idle";
     2, ro, both, TXRDY,      "Transmitter ready";
     3, ro, both, TXIDLE,     "Transmitter idle";
     4, ro, flag, CTS,        "CTS signal asserted";
     5, w1, both, DELTACTS,   "Change of CTS signal detected";
     6, ro, both, TXDIS,      "Transmitter disabled";
     8, w1, both, OVERRUN,    "Overrun error";
    10, ro, flag, RXBRK,      "Received break";
    11, w1, both, DELTARXBRK, "RXBRK signal has changed state";
    12, w1, both, START,      "Start detected";
    13, w1, both, FRAMERR,    "Framing error";
    14, w1, both, PARITYERR,  "Parity error";
    15, w1, both, RXNOISE,    "Received noise";
    16, w1, both, ABERR,      "Autobaud error";
);
