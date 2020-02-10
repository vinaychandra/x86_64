//! Functions to read and write model specific registers.

use crate::registers::rflags::RFlags;
use crate::structures::gdt::SegmentSelector;
use crate::PrivilegeLevel;
use bit_field::BitField;
use bitflags::bitflags;
use core::convert::TryInto;

/// A model specific register.
#[derive(Debug)]
pub struct Msr(u32);

impl Msr {
    /// Create an instance from a register.
    pub const fn new(reg: u32) -> Msr {
        Msr(reg)
    }
}

/// The Extended Feature Enable Register.
#[derive(Debug)]
pub struct Efer;

/// FS.Base Model Specific Register.
#[derive(Debug)]
pub struct FsBase;

/// GS.Base Model Specific Register.
#[derive(Debug)]
pub struct GsBase;

/// KernelGsBase Model Specific Register.
#[derive(Debug)]
pub struct KernelGsBase;

/// Syscall Register: STAR
#[derive(Debug)]
pub struct Star;

/// Syscall Register: LSTAR
#[derive(Debug)]
pub struct LStar;

/// Syscall Register: SFMASK
#[derive(Debug)]
pub struct SFMask;

impl Efer {
    /// The underlying model specific register.
    pub const MSR: Msr = Msr(0xC0000080);
}

impl FsBase {
    /// The underlying model specific register.
    pub const MSR: Msr = Msr(0xC000_0100);
}

impl GsBase {
    /// The underlying model specific register.
    pub const MSR: Msr = Msr(0xC000_0101);
}

impl KernelGsBase {
    /// The underlying model specific register.
    pub const MSR: Msr = Msr(0xC000_0102);
}

impl Star {
    /// The underlying model specific register.
    pub const MSR: Msr = Msr(0xC000_0081);
}

impl LStar {
    /// The underlying model specific register.
    pub const MSR: Msr = Msr(0xC000_0082);
}

impl SFMask {
    /// The underlying model specific register.
    pub const MSR: Msr = Msr(0xC000_0084);
}

bitflags! {
    /// Flags of the Extended Feature Enable Register.
    pub struct EferFlags: u64 {
        /// Enables the `syscall` and `sysret` instructions.
        const SYSTEM_CALL_EXTENSIONS = 1 << 0;
        /// Activates long mode, requires activating paging.
        const LONG_MODE_ENABLE = 1 << 8;
        /// Indicates that long mode is active.
        const LONG_MODE_ACTIVE = 1 << 10;
        /// Enables the no-execute page-protection feature.
        const NO_EXECUTE_ENABLE = 1 << 11;
        /// Enables SVM extensions.
        const SECURE_VIRTUAL_MACHINE_ENABLE = 1 << 12;
        /// Enable certain limit checks in 64-bit mode.
        const LONG_MODE_SEGMENT_LIMIT_ENABLE = 1 << 13;
        /// Enable the `fxsave` and `fxrstor` instructions to execute faster in 64-bit mode.
        const FAST_FXSAVE_FXRSTOR = 1 << 14;
        /// Changes how the `invlpg` instruction operates on TLB entries of upper-level entries.
        const TRANSLATION_CACHE_EXTENSION = 1 << 15;
    }
}

#[cfg(target_arch = "x86_64")]
mod x86_64 {
    use super::*;
    use crate::addr::VirtAddr;

    impl Msr {
        /// Read 64 bits msr register.
        pub unsafe fn read(&self) -> u64 {
            let (high, low): (u32, u32);
            asm!("rdmsr" : "={eax}" (low), "={edx}" (high) : "{ecx}" (self.0) : "memory" : "volatile");
            ((high as u64) << 32) | (low as u64)
        }

        /// Write 64 bits to msr register.
        pub unsafe fn write(&mut self, value: u64) {
            let low = value as u32;
            let high = (value >> 32) as u32;
            asm!("wrmsr" :: "{ecx}" (self.0), "{eax}" (low), "{edx}" (high) : "memory" : "volatile" );
        }
    }

    impl Efer {
        /// Read the current EFER flags.
        pub fn read() -> EferFlags {
            EferFlags::from_bits_truncate(Self::read_raw())
        }

        /// Read the current raw EFER flags.
        pub fn read_raw() -> u64 {
            unsafe { Self::MSR.read() }
        }

        /// Write the EFER flags, preserving reserved values.
        ///
        /// Preserves the value of reserved fields. Unsafe because it's possible to break memory
        /// safety, e.g. by disabling long mode.
        pub unsafe fn write(flags: EferFlags) {
            let old_value = Self::read_raw();
            let reserved = old_value & !(EferFlags::all().bits());
            let new_value = reserved | flags.bits();

            Self::write_raw(new_value);
        }

        /// Write the EFER flags.
        ///
        /// Does not preserve any bits, including reserved fields. Unsafe because it's possible to
        /// break memory safety, e.g. by disabling long mode.
        pub unsafe fn write_raw(flags: u64) {
            Self::MSR.write(flags);
        }

        /// Update EFER flags.
        ///
        /// Preserves the value of reserved fields. Unsafe because it's possible to break memory
        /// safety, e.g. by disabling long mode.
        pub unsafe fn update<F>(f: F)
        where
            F: FnOnce(&mut EferFlags),
        {
            let mut flags = Self::read();
            f(&mut flags);
            Self::write(flags);
        }
    }

    impl FsBase {
        /// Read the current FsBase register.
        pub fn read() -> VirtAddr {
            VirtAddr::new(unsafe { Self::MSR.read() })
        }

        /// Write a given virtual address to the FS.Base register.
        pub fn write(address: VirtAddr) {
            unsafe { Self::MSR.write(address.as_u64()) };
        }
    }

    impl GsBase {
        /// Read the current GsBase register.
        pub fn read() -> VirtAddr {
            VirtAddr::new(unsafe { Self::MSR.read() })
        }

        /// Write a given virtual address to the GS.Base register.
        pub fn write(address: VirtAddr) {
            unsafe { Self::MSR.write(address.as_u64()) };
        }
    }

    impl KernelGsBase {
        /// Read the current KernelGsBase register.
        pub fn read() -> VirtAddr {
            VirtAddr::new(unsafe { Self::MSR.read() })
        }

        /// Write a given virtual address to the KernelGsBase register.
        pub fn write(address: VirtAddr) {
            unsafe { Self::MSR.write(address.as_u64()) };
        }
    }

    impl Star {
        /// Read the Ring 0 and Ring 3 segment bases.
        /// The remaining fields are ignored because they are
        /// not valid for long mode.
        ///
        /// # Returns
        /// - Field 1 (SYSRET): The CS selector is set to this field + 16. SS.Sel is set to
        /// this field + 8. Because SYSRET always returns to CPL 3, the
        /// RPL bits 1:0 should be initialized to 11b.
        /// - Field 2 (SYSCALL): This field is copied directly into CS.Sel. SS.Sel is set to
        ///  this field + 8. Because SYSCALL always switches to CPL 0, the RPL bits
        /// 33:32 should be initialized to 00b.
        pub fn read_raw() -> (u16, u16) {
            let msr_value = unsafe { Self::MSR.read() };
            let sysret = msr_value.get_bits(48..64);
            let syscall = msr_value.get_bits(32..48);
            (sysret.try_into().unwrap(), syscall.try_into().unwrap())
        }

        /// Read the Ring 0 and Ring 3 segment bases.
        /// Returns
        /// - CS Selector SYSRET
        /// - SS Selector SYSRET
        /// - CS Selector SYSCALL
        /// - SS Selector SYSCALL
        pub fn read() -> (
            SegmentSelector,
            SegmentSelector,
            SegmentSelector,
            SegmentSelector,
        ) {
            let raw = Self::read_raw();
            return (
                SegmentSelector((raw.0 + 16).try_into().unwrap()),
                SegmentSelector((raw.0 + 8).try_into().unwrap()),
                SegmentSelector((raw.1).try_into().unwrap()),
                SegmentSelector((raw.1 + 8).try_into().unwrap()),
            );
        }

        /// Write the Ring 0 and Ring 3 segment bases.
        /// The remaining fields are ignored because they are
        /// not valid for long mode.
        ///
        /// # Parameters
        /// - sysret: The CS selector is set to this field + 16. SS.Sel is set to
        /// this field + 8. Because SYSRET always returns to CPL 3, the
        /// RPL bits 1:0 should be initialized to 11b.
        /// - syscall: This field is copied directly into CS.Sel. SS.Sel is set to
        ///  this field + 8. Because SYSCALL always switches to CPL 0, the RPL bits
        /// 33:32 should be initialized to 00b.
        ///
        /// # Unsafety
        /// Unsafe because this can cause system instability if passed in the
        /// wrong values for the fields.
        pub unsafe fn write_raw(sysret: u16, syscall: u16) {
            let mut msr_value = 0u64;
            msr_value.set_bits(48..64, sysret.into());
            msr_value.set_bits(32..48, syscall.into());
            Self::MSR.write(msr_value);
        }

        /// Write the Ring 0 and Ring 3 segment bases.
        /// The remaining fields are ignored because they are
        /// not valid for long mode.
        /// This function will fail if the segment selectors are
        /// not in the correct offset of each other or if the
        /// segment selectors do not have correct privileges.
        pub fn write(
            cs_sysret: SegmentSelector,
            ss_sysret: SegmentSelector,
            cs_syscall: SegmentSelector,
            ss_syscall: SegmentSelector,
        ) -> Result<(), &'static str> {
            if cs_sysret.0 - 16 != ss_sysret.0 - 8 {
                return Err("Sysret CS and SS is not offset by 8.");
            }

            if cs_syscall.0 != ss_syscall.0 - 8 {
                return Err("Syscall CS and SS is not offset by 8.");
            }

            if ss_sysret.rpl() != PrivilegeLevel::Ring3 {
                return Err("Sysret's segment must be a Ring3 segment.");
            }

            if ss_syscall.rpl() != PrivilegeLevel::Ring0 {
                return Err("Syscall's segment must be a Ring0 segment.");
            }

            unsafe { Self::write_raw((ss_sysret.0 - 8).into(), cs_syscall.0.into()) };

            Ok(())
        }
    }

    impl LStar {
        /// Read the current LStar register.
        /// This holds the target RIP of a syscall.
        pub fn read() -> VirtAddr {
            VirtAddr::new(unsafe { Self::MSR.read() })
        }

        /// Write a given virtual address to the LStar register.
        /// This holds the target RIP of a syscall.
        pub fn write(address: VirtAddr) {
            unsafe { Self::MSR.write(address.as_u64()) };
        }
    }

    impl SFMask {
        /// Read to the SFMask register.
        /// The SFMASK register is used to specify which RFLAGS bits
        /// are cleared during a SYSCALL. In long mode, SFMASK is used
        /// to specify which RFLAGS bits are cleared when SYSCALL is
        /// executed. If a bit in SFMASK is set to 1, the corresponding
        /// bit in RFLAGS is cleared to 0. If a bit in SFMASK is cleared
        /// to 0, the corresponding rFLAGS bit is not modified.
        pub fn read() -> RFlags {
            RFlags::from_bits(unsafe { Self::MSR.read() }).unwrap()
        }

        /// Write to the SFMask register.
        /// The SFMASK register is used to specify which RFLAGS bits
        /// are cleared during a SYSCALL. In long mode, SFMASK is used
        /// to specify which RFLAGS bits are cleared when SYSCALL is
        /// executed. If a bit in SFMASK is set to 1, the corresponding
        /// bit in RFLAGS is cleared to 0. If a bit in SFMASK is cleared
        /// to 0, the corresponding rFLAGS bit is not modified.
        pub fn write(value: RFlags) {
            unsafe { Self::MSR.write(value.bits()) };
        }
    }
}
