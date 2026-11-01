//! APK tooling checks this exported constant against the signed AOT manifest.
pub(super) const GUEST_EH:bool=cfg!(feature="android-app-eh");
pub(super) const INLINE_EAGER:bool=cfg!(feature="android-app-inline-eager");
pub(super) const EPOCH_GUARD:bool=cfg!(feature="android-app-epoch");
#[cfg(all(feature="android-app-epoch",feature="android-app-inline-eager"))]
compile_error!("Do not combine epoch and the failed inline/eager experiment");
#[unsafe(no_mangle)]
pub static RAVENDROID_AOT_PROFILE_V1:u32=(GUEST_EH as u32)|((INLINE_EAGER as u32)<<1)|((EPOCH_GUARD as u32)<<2);
