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

.equ DRAM_START,            0x90000000

  .globl main
main:
  la  t0, exception_entry
  csrw    mtvec,t0
  la      t4, DRAM_START
  sw      x0, (t4)
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
payload:
.rept 1024
  .word 0xDEADBEEF
//  .word 0xDEADBEAF
//  .word 0xABABABAB
.endr

