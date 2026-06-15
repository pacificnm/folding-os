package main

import "fmt"

const releaseImageSectorSize = 512

// Approved release-image GPT layout from Milestone 1 engineering specification
// and foldingosctl storage expand-data validation.
const (
	releaseImageEFIPartitionStartSector  = 2048
	releaseImageEFIPartitionSectorCount  = 1048576
	releaseImageRootPartitionStartSector = 1050624
	releaseImageRootPartitionSectorCount = 4194304
)

func copyReleaseImagePartitionFromFile(sourceImage, destination string, startSector, sectorCount uint64) error {
	return run(
		"dd",
		fmt.Sprintf("if=%s", sourceImage),
		fmt.Sprintf("of=%s", destination),
		fmt.Sprintf("bs=%d", releaseImageSectorSize),
		fmt.Sprintf("skip=%d", startSector),
		fmt.Sprintf("count=%d", sectorCount),
		"conv=fsync",
	)
}

func copyStagedReleaseImageEFIPartition(sourceImage, destination string) error {
	return copyReleaseImagePartitionFromFile(
		sourceImage,
		destination,
		releaseImageEFIPartitionStartSector,
		releaseImageEFIPartitionSectorCount,
	)
}

func copyStagedReleaseImageRootPartition(sourceImage, destination string) error {
	return copyReleaseImagePartitionFromFile(
		sourceImage,
		destination,
		releaseImageRootPartitionStartSector,
		releaseImageRootPartitionSectorCount,
	)
}
