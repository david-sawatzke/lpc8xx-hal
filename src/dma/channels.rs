use core::sync::atomic::{compiler_fence, Ordering};

use crate::{
    init_state,
    pac::{
        self,
        dma0::{
            channel::{CFG, XFERCFG},
            ACTIVE0, ENABLESET0, SETTRIG0,
        },
    },
    reg_proxy::{Reg, RegProxy},
};

use super::{
    descriptors::ChannelDescriptor, DescriptorTable, Dest, Handle, Source,
    Transfer,
};

/// A DMA channel
pub struct Channel<C, S>
where
    C: ChannelTrait,
{
    ty: C,
    _state: S,
    descriptor: &'static mut ChannelDescriptor,

    // This channel's dedicated registers.
    cfg: RegProxy<C::Cfg>,
    xfercfg: RegProxy<C::Xfercfg>,

    // Shared registers. We restrict our access to the one bit that is dedicated
    // to this channel, so sharing those with other channels should be safe.
    pub(super) active0: RegProxy<ACTIVE0>,
    enableset0: RegProxy<ENABLESET0>,
    settrig0: RegProxy<SETTRIG0>,
}

impl<C> Channel<C, init_state::Disabled>
where
    C: ChannelTrait,
{
    /// Enable the channel
    pub fn enable<'dma>(
        self,
        dma: &'dma Handle,
    ) -> Channel<C, init_state::Enabled<&'dma Handle>> {
        Channel {
            ty: self.ty,
            _state: init_state::Enabled(dma),
            descriptor: self.descriptor,

            cfg: self.cfg,
            xfercfg: self.xfercfg,

            active0: self.active0,
            enableset0: self.enableset0,
            settrig0: self.settrig0,
        }
    }
}

impl<'dma, C> Channel<C, init_state::Enabled<&'dma Handle>>
where
    C: ChannelTrait,
{
    /// Starts a DMA transfer
    ///
    /// # Panics
    ///
    /// Panics, if any buffer passed to this function has a length larger than
    /// 1024.
    ///
    /// # Limitations
    ///
    /// The caller must make sure to call this method only for the correct
    /// combination of channel and target.
    pub(crate) fn start_transfer<S, D>(
        self,
        source: S,
        mut dest: D,
    ) -> Transfer<'dma, C, S, D>
    where
        S: Source,
        D: Dest,
    {
        assert!(source.is_valid());
        assert!(dest.is_valid());

        compiler_fence(Ordering::SeqCst);

        // To compute the transfer count, source or destination buffers need to
        // subtract 1 from their length. This early return makes sure that
        // this won't lead to an underflow.
        if source.is_empty() || dest.is_full() {
            return Transfer::new(self, source, dest);
        }

        // Currently we don't support memory-to-memory transfers, which means
        // exactly one participant is providing the transfer count.
        let source_count = source.transfer_count();
        let dest_count = dest.transfer_count();
        let transfer_count = match (source_count, dest_count) {
            (Some(transfer_count), None) => transfer_count,
            (None, Some(transfer_count)) => transfer_count,
            _ => {
                panic!("Unsupported transfer type");
            }
        };

        // Configure channel
        // See user manual, section 12.6.16.
        self.cfg.write(|w| {
            w.periphreqen().enabled();
            w.hwtrigen().disabled();
            unsafe { w.chpriority().bits(0) }
        });

        // Set channel transfer configuration
        // See user manual, section 12.6.18.
        self.xfercfg.write(|w| {
            w.cfgvalid().valid();
            w.reload().disabled();
            w.swtrig().not_set();
            w.clrtrig().cleared();
            w.setinta().no_effect();
            w.setintb().no_effect();
            w.width().bit_8();
            w.srcinc().variant(source.increment());
            w.dstinc().variant(dest.increment());
            unsafe { w.xfercount().bits(transfer_count) }
        });

        // Configure channel descriptor
        // See user manual, sections 12.5.2 and 12.5.3.
        self.descriptor.source_end = source.end_addr();
        self.descriptor.dest_end = dest.end_addr();

        // Enable channel
        // See user manual, section 12.6.4.
        self.enableset0.write(|w| unsafe { w.ena().bits(C::FLAG) });

        // Trigger transfer
        self.settrig0.write(|w| unsafe { w.trig().bits(C::FLAG) });

        Transfer::new(self, source, dest)
    }
}

/// Implemented for each DMA channel
pub trait ChannelTrait {
    /// The index of the channel
    ///
    /// This is `0` for channel 0, `1` for channel 1, etc.
    const INDEX: usize;

    /// The flag for the channel
    ///
    /// This is `0x1` for channel 0, `0x2` for channel 2, `0x4` for channel 3,
    /// etc.
    const FLAG: u32;

    /// The type that represents this channel's CFG register
    type Cfg: Reg<Target = CFG>;

    /// The type that represents this channel's XFERCFG register
    type Xfercfg: Reg<Target = XFERCFG>;
}

macro_rules! channels {
    ($($field:ident, $name:ident, $index:expr, $cfg:ident, $xfercfg:ident;)*) => {
        /// Provides access to all channels
        #[allow(missing_docs)]
        pub struct Channels {
            $(pub $field: Channel<$name, init_state::Disabled>,)*
        }

        impl Channels {
            pub(super) fn new(descriptors: &'static mut DescriptorTable)
                -> Self
            {
                let mut descriptors = (&mut descriptors.0).into_iter();

                Channels {
                    $(
                        $field: Channel {
                            ty        : $name(()),
                            _state    : init_state::Disabled,
                            descriptor: descriptors.next().unwrap(),

                            cfg    : RegProxy::new(),
                            xfercfg: RegProxy::new(),

                            active0   : RegProxy::new(),
                            enableset0: RegProxy::new(),
                            settrig0  : RegProxy::new(),
                        },
                    )*
                }
            }
        }


        $(
            /// This struct is an implementation detail that shouldn't be used by user
            pub struct $xfercfg;

            reg_cluster!($xfercfg, XFERCFG, pac::DMA0, $field, xfercfg);

            /// This struct is an implementation detail that shouldn't be used by user
            pub struct $cfg;

            reg_cluster!($cfg, CFG, pac::DMA0, $field, cfg);

            /// Identifies a DMA channel
            pub struct $name(());

            impl ChannelTrait for $name {
                const INDEX: usize = $index;
                const FLAG : u32   = 0x1 << Self::INDEX;

                type Cfg     = $cfg;
                type Xfercfg = $xfercfg;
            }
        )*
    }
}

#[cfg(feature = "82x")]
// The channels must always be specified in order, from lowest to highest, as
// the channel descriptors are assigned based on that order.
channels!(
    channel0 , Channel0 ,  0, CFG0 , XFERCFG0 ;
    channel1 , Channel1 ,  1, CFG1 , XFERCFG1 ;
    channel2 , Channel2 ,  2, CFG2 , XFERCFG2 ;
    channel3 , Channel3 ,  3, CFG3 , XFERCFG3 ;
    channel4 , Channel4 ,  4, CFG4 , XFERCFG4 ;
    channel5 , Channel5 ,  5, CFG5 , XFERCFG5 ;
    channel6 , Channel6 ,  6, CFG6 , XFERCFG6 ;
    channel7 , Channel7 ,  7, CFG7 , XFERCFG7 ;
    channel8 , Channel8 ,  8, CFG8 , XFERCFG8 ;
    channel9 , Channel9 ,  9, CFG9 , XFERCFG9 ;
    channel10, Channel10, 10, CFG10, XFERCFG10;
    channel11, Channel11, 11, CFG11, XFERCFG11;
    channel12, Channel12, 12, CFG12, XFERCFG12;
    channel13, Channel13, 13, CFG13, XFERCFG13;
    channel14, Channel14, 14, CFG14, XFERCFG14;
    channel15, Channel15, 15, CFG15, XFERCFG15;
    channel16, Channel16, 16, CFG16, XFERCFG16;
    channel17, Channel17, 17, CFG17, XFERCFG17;
);

#[cfg(feature = "845")]
// The channels must always be specified in order, from lowest to highest, as
// the channel descriptors are assigned based on that order.
channels!(
    channel0 , Channel0 ,  0, CFG0 , XFERCFG0 ;
    channel1 , Channel1 ,  1, CFG1 , XFERCFG1 ;
    channel2 , Channel2 ,  2, CFG2 , XFERCFG2 ;
    channel3 , Channel3 ,  3, CFG3 , XFERCFG3 ;
    channel4 , Channel4 ,  4, CFG4 , XFERCFG4 ;
    channel5 , Channel5 ,  5, CFG5 , XFERCFG5 ;
    channel6 , Channel6 ,  6, CFG6 , XFERCFG6 ;
    channel7 , Channel7 ,  7, CFG7 , XFERCFG7 ;
    channel8 , Channel8 ,  8, CFG8 , XFERCFG8 ;
    channel9 , Channel9 ,  9, CFG9 , XFERCFG9 ;
    channel10, Channel10, 10, CFG10, XFERCFG10;
    channel11, Channel11, 11, CFG11, XFERCFG11;
    channel12, Channel12, 12, CFG12, XFERCFG12;
    channel13, Channel13, 13, CFG13, XFERCFG13;
    channel14, Channel14, 14, CFG14, XFERCFG14;
    channel15, Channel15, 15, CFG15, XFERCFG15;
    channel16, Channel16, 16, CFG16, XFERCFG16;
    channel17, Channel17, 17, CFG17, XFERCFG17;
    channel18, Channel18, 18, CFG18, XFERCFG18;
    channel19, Channel19, 19, CFG19, XFERCFG19;
    channel20, Channel20, 20, CFG20, XFERCFG20;
    channel21, Channel21, 21, CFG21, XFERCFG21;
    channel22, Channel22, 22, CFG22, XFERCFG22;
    channel23, Channel23, 23, CFG23, XFERCFG23;
    channel24, Channel24, 24, CFG24, XFERCFG24;
);

reg!(ACTIVE0, ACTIVE0, pac::DMA0, active0);
reg!(ENABLESET0, ENABLESET0, pac::DMA0, enableset0);
reg!(SETTRIG0, SETTRIG0, pac::DMA0, settrig0);