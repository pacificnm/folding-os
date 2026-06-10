package main

import "testing"

func TestParseTable(t *testing.T) {
	table := `Disk /dev/vda: 8388608 sectors
First usable sector is 34, last usable sector is 8388574
Number  Start (sector)    End (sector)  Size       Code  Name
   1            2048         1050623   512.0 MiB   EF00  FOLDINGOS_EFI
   2         1050624         5244927   2.0 GiB     8304  FOLDINGOS_ROOT
   3         5244928         8386559   1.5 GiB     8300  FOLDINGOS_DATA
`

	lastUsable, partitions, err := parseTable(table)
	if err != nil {
		t.Fatal(err)
	}
	if lastUsable != 8388574 {
		t.Fatalf("last usable sector = %d", lastUsable)
	}
	if partitions[3].start != dataPartitionStart || partitions[3].end != 8386559 {
		t.Fatalf("unexpected data partition: %+v", partitions[3])
	}
}

func TestAlignedEnd(t *testing.T) {
	if got := alignedEnd(8388574); got != 8386559 {
		t.Fatalf("aligned end = %d", got)
	}
	if got := alignedEnd(12582878); got != 12580863 {
		t.Fatalf("larger aligned end = %d", got)
	}
}

func TestPartitionDevice(t *testing.T) {
	tests := map[string]string{
		"/dev/vda":     "/dev/vda3",
		"/dev/sda":     "/dev/sda3",
		"/dev/nvme0n1": "/dev/nvme0n1p3",
		"/dev/mmcblk0":  "/dev/mmcblk0p3",
	}
	for disk, expected := range tests {
		if got := partitionDevice(disk, dataPartitionNumber); got != expected {
			t.Fatalf("partitionDevice(%q) = %q, want %q", disk, got, expected)
		}
	}
}
