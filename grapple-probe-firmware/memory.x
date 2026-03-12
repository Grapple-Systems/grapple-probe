MEMORY {
    BOOT2   : ORIGIN = 0x10000000, LENGTH = 0x100
    FLASH   : ORIGIN = 0x10000100, LENGTH = 1024K - 0x100 - 8K
    STORAGE : ORIGIN = 0x100FE000, LENGTH = 8K

    /* Pick one of the two options for RAM layout     */

    /* OPTION A: Use all RAM banks as one big block   */
    /* Reasonable, unless you are doing something     */
    /* really particular with DMA or other concurrent */
    /* access that would benefit from striping        */
    RAM   : ORIGIN = 0x20000000, LENGTH = 264K

    /* OPTION B: Keep the unstriped sections separate */
    /* RAM: ORIGIN = 0x20000000, LENGTH = 256K        */
    /* SCRATCH_A: ORIGIN = 0x20040000, LENGTH = 4K    */
    /* SCRATCH_B: ORIGIN = 0x20041000, LENGTH = 4K    */
}

__partition_storage_start = ORIGIN(STORAGE) - ORIGIN(BOOT2);
__partition_storage_end = ORIGIN(STORAGE) + LENGTH(STORAGE) - ORIGIN(BOOT2);