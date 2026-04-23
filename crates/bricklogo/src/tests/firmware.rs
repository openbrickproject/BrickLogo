use super::*;

#[test]
fn test_chip_from_iproduct() {
    assert_eq!(SpikeChip::from_iproduct("STM32F413"), Some(SpikeChip::F4));
    assert_eq!(SpikeChip::from_iproduct("STM32H562"), Some(SpikeChip::H5));
    assert_eq!(SpikeChip::from_iproduct("STM32  BOOTLOADER"), None);
}

#[test]
fn test_chip_from_interface_desc() {
    // STM32F413 (SPIKE Prime 45678) layout from the ST DFU bootloader.
    let f4 = "@Internal Flash  /0x08000000/04*016Kg,01*064Kg,07*128Kg";
    assert_eq!(SpikeChip::from_interface_desc(f4), Some(SpikeChip::F4));
    // STM32H562 (MINDSTORMS 51515).
    let h5 = "@Internal Flash   /0x08000000/128*008Kg";
    assert_eq!(SpikeChip::from_interface_desc(h5), Some(SpikeChip::H5));
}

#[test]
fn test_chip_filenames_exist_in_bundled() {
    let dir = bundled_dir().join("spike-prime");
    if !dir.is_dir() {
        return;
    }
    for chip in [SpikeChip::F4, SpikeChip::H5] {
        let gz = dir.join(chip.dfuse_gz_filename());
        assert!(gz.exists(), "missing bundled dfuse.gz: {}", gz.display());
    }
}

#[test]
fn test_buildhat_signature_path_derivation() {
    use std::path::PathBuf;
    let p = PathBuf::from("/opt/bl/firmware/buildhat/buildhat-firmware-1902784.bin");
    let sig = derive_buildhat_signature_path(&p).unwrap();
    assert_eq!(
        sig,
        PathBuf::from("/opt/bl/firmware/buildhat/buildhat-signature-1902784.bin")
    );
}

#[test]
fn test_buildhat_signature_derivation_rejects_non_conforming_name() {
    use std::path::PathBuf;
    assert!(derive_buildhat_signature_path(&PathBuf::from("/tmp/custom.bin")).is_none());
}

#[test]
fn test_bundled_dir_defaults_to_firmware() {
    // Regardless of execution location, bundled_dir should always return
    // something — either a real path next to the binary or the fallback.
    let dir = bundled_dir();
    assert!(!dir.as_os_str().is_empty());
}
