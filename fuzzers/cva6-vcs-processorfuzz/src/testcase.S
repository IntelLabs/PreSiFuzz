# Copyright 2022 Thales DIS design services SAS
#
# Licensed under the Solderpad Hardware Licence, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# SPDX-License-Identifier: Apache-2.0 WITH SHL-2.0
# You may obtain a copy of the License at https://solderpad.org/licenses/
#
# Original Author: Guillaume Chauvon (guillaume.chauvon@thalesgroup.fr)

#*****************************************************************************
# custom_test_template.S
#-----------------------------------------------------------------------------
#
DRAM_START:
  .word 0x90000000

.align 2
  .globl _start
  .section ".text.init"
_start:
  la  t0, exception_entry
  csrw    mtvec,t0
  la      t4, DRAM_START
  sw      x0, (t4)

  la  x1, DRAM_START
  la  x2, DRAM_START
  la  x3, DRAM_START
  la  x4, DRAM_START
  la  x5, DRAM_START
  la  x6, DRAM_START
  la  x7, DRAM_START
  la  x8, DRAM_START
  la  x9, DRAM_START
  la  x10,DRAM_START
  la  x11,DRAM_START
  la  x12,DRAM_START
  la  x13,DRAM_START
  la  x14,DRAM_START
  la  x15,DRAM_START
  la  x16,DRAM_START
  la  x17,DRAM_START
  la  x18,DRAM_START
  la  x19,DRAM_START
  la  x20,DRAM_START
  la  x21,DRAM_START
  la  x22,DRAM_START
  la  x23,DRAM_START
  la  x24,DRAM_START
  la  x25,DRAM_START
  la  x26,DRAM_START
  la  x27,DRAM_START
  la  x28,DRAM_START
  la  x29,DRAM_START
  la  x30,DRAM_START
  la  x31,DRAM_START

  jal     x0, payload

.align 2
exception_entry:
        csrr t0, mepc
        lb t1, 0(t0)
        li a0, 0x3
        and t1, t1, a0

        // stop the fuzzer if counter reaches threshold
        la    t4, DRAM_START
        lw    t5, (t4)
        add   t5, t5, 1
        li    t6, 10
        beq   t5, t6, exit
        sw    t5, (t4)
        /* Increment mepc by 2 or 4 depending on whether the instruction at mepc
           is compressed or not.  */
        bne t1, a0, end_handler_incr_mepc2
        addi t0, t0, 2
end_handler_incr_mepc2:
        addi t0, t0, 2
        csrw mepc, t0
end_handler_ret:
        mret

//csrr  t3, mepc
//csrr  t4, 0xfC2 #CSR_MSTATUS_REG_ADDR
//andi  t4, t4, 0x00000080 #MSTATUS_IL_MASK
//srli  t4, t4, 7 #MSTATUS_IL_SHIFT
//slli  t4, t4, 1 #*2
//add   t3, t3, t4
//add   t3, t3, 2
//csrw  mepc, t3
//la    t4, DRAM_START
//lw    t5, (t4)
//add   t5, t5, 1
//li    t6, 100
//beq   t5, t6, exit
//sw    t5, (t4)
//mret
//lui     t0,0x101
//lui     t0,0x101
//lui     t0,0x101
//lui     t0,0x101
//lui     t0,0x101
//lui     t0,0x101
//lui     t0,0x101
//lui     t0,0x101
//lui     t0,0x101

.align 4
exit:
  la  t3, end
  csrw  sepc, t3
  mret
.align 4
end: 
  wfi
  jal x0, end

.globl payload
.align 4
//  .section ".text"
payload:
.word 0x00000013
.word 0x00000013
.word 0x00000013
.word 0x00000013
.word 0x00000013
.word 0x00000013
.word 0x00000013
.word 0x00000013
.word 0x00000013
.word 0x00000013
.word 0x00000013
.word 0x00000013
.word 0x00000013
.word 0x00000013
.word 0x00000013
.word 0x00000013
.rept 17000
  .word 0xDEADBEEF
//  .word 0xDEADBEAF
//  .word 0xABABABAB
.endr
.word 0x00000013
.word 0x00000013
.word 0x00000013
.word 0x00000013
.word 0x00000013
.word 0x00000013
.word 0x00000013
.word 0x00000013
.word 0x00000013
.word 0x00000013
.word 0x00000013
.word 0x00000013

.globl code
.align 4
.section ".text"
.rept 17000
  .word 0xDEADBEEF
.endr
