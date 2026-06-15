package main

import "testing"

func TestReleaseImagePartitionLayoutMatchesApprovedGPT(t *testing.T) {
	if releaseImageEFIPartitionStartSector+releaseImageEFIPartitionSectorCount != releaseImageRootPartitionStartSector {
		t.Fatalf("EFI partition does not end where root partition starts")
	}
	if releaseImageRootPartitionStartSector+releaseImageRootPartitionSectorCount != dataPartitionStart {
		t.Fatalf("root partition does not end where data partition starts")
	}
	if releaseImageEFIPartitionSectorCount*releaseImageSectorSize != 512*1024*1024 {
		t.Fatalf("EFI partition size is not 512 MiB")
	}
	if releaseImageRootPartitionSectorCount*releaseImageSectorSize != 2*1024*1024*1024 {
		t.Fatalf("root partition size is not 2 GiB")
	}
}
