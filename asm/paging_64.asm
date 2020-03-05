    ; Initialize PML4
    MOV                  EAX, 0

    PML4                 EQU 0x00100000
    MOV                  EDI, PML4

    ; 1 PML4, 2 PDPT, 2PD and 2PT
    BYTES_PML4           EQU 1024
    BYTES_PDPT           EQU 1024
    BYTES_PD             EQU 1024
    BYTES_PT             EQU 1024
    NUM_ALL_ENTRIES      EQU BYTES_PML4 + 2 * BYTES_PDPT + 2 * BYTES_PD + 2 * BYTES_PT
    MOV                  ECX, NUM_ALL_ENTRIES

    REP                  STOSD

    ; Add a PML4 entry for below 1MB
    PDPT_BELOW_1MB       EQU PML4 + BYTES_PML4
    PAGE_EXISTS          EQU 1

    ; MOV [DWORD PML4] will cause an assemble error.
    ; MOV DWORD[PML4] won't cause any assemble errors, but it won't assign a value
    ; to ES:PML4.
    MOV                  DWORD[DWORD PML4], PDPT_BELOW_1MB | PAGE_EXISTS

    ; Add a PDPT entry for below 1MB
    PD_BELOW_1MB         EQU PDPT_BELOW_1MB + BYTES_PDPT
    MOV                  DWORD[DWORD PDPT_BELOW_1MB], PD_BELOW_1MB | PAGE_EXISTS

    ; Add a PD entry and PT entries for below 1MB
    MOV                  EAX, 0
    MOV                  EBX, PT_BELOW_1MB

    PT_BELOW_1MB         EQU PD_BELOW_1MB + BYTES_PD
    MOV                  EDI, PD_BELOW_1MB
    MOV                  ECX, 1024 * 1024
    CALL                 map_entries

    ; Add a PML4 entry for kernel
    PML4_ENTRY_KERNEL    EQU PML4 + 0x1FF << 3
    PDPT_KERNEL          EQU PT_BELOW_1MB + BYTES_PT
    MOV                  DWORD[DWORD PML4_ENTRY_KERNEL], PDPT_KERNEL | PAGE_EXISTS

    ; Add a PDPT entry for kernel
    PDPT_ENTRY_KERNEL    EQU PDPT_KERNEL + 0x1FF << 3
    PD_KERNEL            EQU PDPT_KERNEL + BYTES_PDPT
    MOV                  DWORD[DWORD PDPT_ENTRY_KERNEL], PD_KERNEL | PAGE_EXISTS

    ; Functions

map_entries:
    ; Associate physical memories starting with EAX to page directory entries
    ; starting with EDI.
    ; Page table will be created successively from physical address EBX.
    ; EDX will be used as a temporary register.
    ;
    ; EAX: Starting address of physical memories.
    ; EBX: Starting address of page tables.
    ; EDI: Starting address of entries of a page directory.
    ; ECX: Number of bytes to map.
    PUSH                 EBP
    MOV                  EBP, ESP

    ; Number of entries = ECX / (bytes of a page table)
    ;                   = ECX >> 12
    SHR                  ECX, 12

loop_map_entries:
    ; The number of entries a 4-level page table contains is 512, not 1024.
    NUM_PAGE_ENTRIES     EQU 512
    CMP                  ECX, NUM_PAGE_ENTRIES
    JBE                  map_remainings

    MOV                  EDX, EBX
    OR                   EDX, PAGE_EXISTS
    MOV                  [EDI], EDX

    PUSH                 ECX,
    MOV                  ECX, NUM_PAGE_ENTRIES

    PUSH                 EDI
    MOV                  EDI, EBX
    CALL                 map_to_single_table

    POP                  EDI
    POP                  ECX

    SUB                  ECX, NUM_PAGE_ENTRIES

    SIZE_TABLE           EQU 0x1000
    ADD                  EBX, SIZE_TABLE

    ; The size of entry of 4-level paging is 8 bytes, not 4.
    SIZE_ENTRY           EQU 8
    ADD                  EDI, SIZE_ENTRY

    JMP                  loop_map_entries

map_remainings:
    MOV                  EDX, EBX
    OR                   EDX, PAGE_EXISTS
    MOV                  [EDI], EDX

    MOV                  EDI, EBX
    CALL                 map_to_single_table

    MOV                  ESP, EBP
    POP                  EBP
    RET

map_to_single_table:
    ; Map ECX entries to a page table.
    ; (EDI - (page directory base address)) / 4 + ECX must be less than or equal to 1024.
    ;
    ; EAX: Starting address of physical memories.
    ; ECX: Number of entries to map.
    ; EDI: Starting address of entries of a page table.
    ; EDX will be used as a temporary register.
    PUSH                 EBP
    MOV                  EBP, ESP

loop_map_to_single_table:
    CMP                  ECX, 0
    JBE                  end_map_to_single_table

    MOV                  EDX, EAX
    OR                   EDX, PAGE_EXISTS
    MOV                  [EDI], EDX

    ADD                  EAX, 0x1000
    ADD                  EDI, SIZE_ENTRY
    DEC                  ECX

    JMP                  loop_map_to_single_table

end_map_to_single_table:
    MOV                  ESP, EBP
    POP                  EBP
    RET

