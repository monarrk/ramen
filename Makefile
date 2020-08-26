RUST_SRC_DIR	:= src
BUILD_DIR		:= build
EFI_DIR		:= bootx64
EFI_SRC_DIR		:= $(EFI_DIR)/$(RUST_SRC_DIR)
CLIB_DIR		:= c_lib
COMMON_SRC_DIR	:= common_items

CARGO_JSON		:= cargo_settings
RUST_SRC		:= $(shell cd $(RUST_SRC_DIR) && ls)
EFI_SRC			:= $(shell find $(EFI_DIR) -name '*.rs')

COMMON_SRC		:= $(addprefix $(COMMON_SRC_DIR)/$(RUST_SRC_DIR)/, $(shell ls $(COMMON_SRC_DIR)/$(RUST_SRC_DIR)))

LD_SRC			:= os.ld
CLIB_SRC		:= $(CLIB_DIR)/lib.c

EFI_FILE		:= $(BUILD_DIR)/bootx64.efi

KERNEL_FILE		:= $(BUILD_DIR)/kernel.bin
LIB_FILE		:= $(BUILD_DIR)/libramen_os.a
IMG_FILE		:= $(BUILD_DIR)/ramen_os.img
CLIB_FILE		:= $(BUILD_DIR)/clib.o

CAT				:= cat
LD				:= ld
CC				:= gcc
RUSTCC			:= cargo
RM				:= rm -rf
VIEWER			:= qemu-system-x86_64

OVMF_CODE		:= OVMF_CODE.fd
OVMF_VARS		:= OVMF_VARS.fd

CFLAGS			:= -O3 -pipe -nostdlib -c -ffreestanding
VIEWERFLAGS		:= -drive if=pflash,format=raw,file=$(OVMF_CODE),readonly=on -drive if=pflash,format=raw,file=$(OVMF_VARS),readonly=on -drive format=raw,file=$(IMG_FILE) -monitor stdio -no-reboot -no-shutdown -m 4G -d int

LDFLAGS			:= -nostdlib -T $(LD_SRC)

.PHONY:all copy_to_usb run release clean

.SUFFIXES:

all:$(KERNEL_FILE) $(HEAD_FILE) $(EFI_FILE)

copy_to_usb:$(KERNEL_FILE) $(HEAD_FILE) $(EFI_FILE)
ifeq ($(USB_DEVICE_PATH),)
	echo 'Specify device path by $$USB_DEVICE_PATH environment variable.' >&2
else
	sudo mount $(USB_DEVICE_PATH) /mnt
	sudo mkdir -p /mnt/efi/boot
	sudo cp $(EFI_FILE) /mnt/efi/boot/
	sudo cp $(KERNEL_FILE) /mnt/
	sudo umount /mnt
endif

run:$(IMG_FILE) $(OVMF_VARS) $(OVMF_CODE)
	$(VIEWER) $(VIEWERFLAGS)

$(IMG_FILE):$(KERNEL_FILE) $(HEAD_FILE) $(EFI_FILE)
	dd if=/dev/zero of=$@ bs=1k count=28800
	mformat -i $@ -h 200 -t 500 -s 144::
	# Cannot replace these mmd and mcopy with `make copy_to_usb` because `mount` needs `sudo`
	# regardless of the permission of the image file or the device. Using `mmd` and `mcopy` is
	# the only way to edit image file without `sudo`.
	mmd -i $@ ::/efi
	mmd -i $@ ::/efi/boot
	mcopy -i $@ $(KERNEL_FILE) ::
	mcopy -i $@ $(EFI_FILE) ::/efi/boot

release:
	make clean && make RELEASE_FLAGS=--release

$(KERNEL_FILE):$(LIB_FILE) $(CLIB_FILE) $(LD_SRC)|$(BUILD_DIR)
	$(LD) $(LDFLAGS) -o $@ $(LIB_FILE) $(CLIB_FILE)

$(LIB_FILE): $(addprefix $(RUST_SRC_DIR)/, $(RUST_SRC)) $(COMMON_SRC)|$(BUILD_DIR)
	$(RUSTCC) build --out-dir $(BUILD_DIR) -Z unstable-options $(RELEASE_FLAGS)

$(CLIB_FILE):$(CLIB_SRC)|$(BUILD_DIR)
	$(CC) $(CFLAGS) -o $@ $<

%.fd:
	@echo "$@ not found"
	exit 1

$(EFI_FILE):$(EFI_SRC) $(COMMON_SRC)
	$(RUSTCC) build --target=x86_64-unknown-uefi --manifest-path=$(EFI_DIR)/Cargo.toml --out-dir=$(BUILD_DIR) -Z unstable-options $(RELEASE_FLAGS)

$(BUILD_DIR):
	mkdir $@

clean:
	$(RM) build
	$(RUSTCC) clean
	$(RUSTCC) clean --manifest-path=$(EFI_DIR)/Cargo.toml
