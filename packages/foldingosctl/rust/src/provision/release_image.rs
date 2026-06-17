use crate::provision::util::run_command;

const RELEASE_IMAGE_SECTOR_SIZE: u64 = 512;
const RELEASE_IMAGE_EFI_PARTITION_START_SECTOR: u64 = 2048;
const RELEASE_IMAGE_EFI_PARTITION_SECTOR_COUNT: u64 = 1_048_576;
const RELEASE_IMAGE_ROOT_PARTITION_START_SECTOR: u64 = 1_050_624;
const RELEASE_IMAGE_ROOT_PARTITION_SECTOR_COUNT: u64 = 4_194_304;

fn copy_release_image_partition_from_file(
    source_image: &str,
    destination: &str,
    start_sector: u64,
    sector_count: u64,
) -> Result<(), String> {
    run_command(
        "dd",
        &[
            &format!("if={source_image}"),
            &format!("of={destination}"),
            &format!("bs={RELEASE_IMAGE_SECTOR_SIZE}"),
            &format!("skip={start_sector}"),
            &format!("count={sector_count}"),
            "conv=fsync",
        ],
    )
}

pub fn copy_staged_release_image_efi_partition(
    source_image: &str,
    destination: &str,
) -> Result<(), String> {
    copy_release_image_partition_from_file(
        source_image,
        destination,
        RELEASE_IMAGE_EFI_PARTITION_START_SECTOR,
        RELEASE_IMAGE_EFI_PARTITION_SECTOR_COUNT,
    )
}

pub fn copy_staged_release_image_root_partition(
    source_image: &str,
    destination: &str,
) -> Result<(), String> {
    copy_release_image_partition_from_file(
        source_image,
        destination,
        RELEASE_IMAGE_ROOT_PARTITION_START_SECTOR,
        RELEASE_IMAGE_ROOT_PARTITION_SECTOR_COUNT,
    )
}
