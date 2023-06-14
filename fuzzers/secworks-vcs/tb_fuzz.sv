//======================================================================
//
// tb_fuzz.v
// -----------
// Testbench for the fuzzing SHA-256.
//
// Modified by: Nassim Corteggiani
// Based on tb_sha256.sv from https://github.com/secworks/sha256
// SPDX-FileCopyrightText: 2022 Intel Corporation
// SPDX-License-Identifier: BSD-2-Clause
//
// Redistribution and use in source and binary forms, with or
// without modification, are permitted provided that the following
// conditions are met:
//
// 1. Redistributions of source code must retain the above copyright
//    notice, this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright
//    notice, this list of conditions and the following disclaimer in
//    the documentation and/or other materials provided with the
//    distribution.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
// "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
// LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS
// FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE
// COPYRIGHT OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT,
// INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;
// LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF
// ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
//======================================================================

`default_nettype none

module tb_fuzz();

  parameter             WIDTH = 32;
  
  int fd;
  int status;
  string test_case;

  //----------------------------------------------------------------
  // Internal constant and parameter definitions.
  //----------------------------------------------------------------
  parameter DEBUG = 0;

  parameter CLK_HALF_PERIOD = 2;
  parameter CLK_PERIOD = 2 * CLK_HALF_PERIOD;

  //----------------------------------------------------------------
  // Register and Wire declarations.
  //----------------------------------------------------------------
  reg           tb_clk;
  reg           tb_reset_n;
  reg           tb_cs;
  reg           tb_we;
  reg [7 : 0]   tb_address;
  reg [31 : 0]  tb_write_data;
  wire [31 : 0] tb_read_data;
  wire          tb_error;

  reg [31 : 0]  read_data;
  reg [255 : 0] digest_data;

  logic [32-1:0]address;
  logic [WIDTH-1:0]data;

  //----------------------------------------------------------------
  // Device Under Test.
  //----------------------------------------------------------------
  sha256 dut(
             .clk(tb_clk),
             .reset_n(tb_reset_n),

             .cs(tb_cs),
             .we(tb_we),


             .address(tb_address),
             .write_data(tb_write_data),
             .read_data(tb_read_data),
             .error(tb_error)
            );


  //----------------------------------------------------------------
  // clk_gen
  //
  // Clock generator process.
  //----------------------------------------------------------------
  always
    begin : clk_gen
      #CLK_HALF_PERIOD tb_clk = !tb_clk;
    end // clk_gen


  //----------------------------------------------------------------
  // sys_monitor
  //
  // Generates a cycle counter and displays information about
  // the dut as needed.
  //----------------------------------------------------------------
  always
    begin : sys_monitor
      #(2 * CLK_HALF_PERIOD);
    end


  //----------------------------------------------------------------
  // reset_dut()
  //
  // Toggles reset to force the DUT into a well defined state.
  //----------------------------------------------------------------
  task reset_dut;
    begin
      tb_reset_n = 0;
      #(4 * CLK_HALF_PERIOD);
      tb_reset_n = 1;
    end
  endtask // reset_dut


  //----------------------------------------------------------------
  // init_sim()
  //
  // Initialize all counters and testbed functionality as well
  // as setting the DUT inputs to defined values.
  //----------------------------------------------------------------
  task init_sim;
    begin
      tb_clk = 0;
      tb_reset_n = 0;
      tb_cs = 0;
      tb_we = 0;
      tb_address = 6'h0;
      tb_write_data = 32'h0;
    end
  endtask // init_dut

  //----------------------------------------------------------------
  // write_word()
  //
  // Write the given word to the DUT using the DUT interface.
  //----------------------------------------------------------------
  task write_word(input [7 : 0]  address,
                  input [31 : 0] word);
    begin
      if (DEBUG)
        begin
          $display("*** Writing 0x%08x to 0x%02x.", word, address);
          $display("");
        end

      tb_address = address;
      tb_write_data = word;
      tb_cs = 1;
      tb_we = 1;
      #(CLK_PERIOD);
      tb_cs = 0;
      tb_we = 0;
    end
  endtask // write_word


  //----------------------------------------------------------------
  // Load fuzz input from file
  //----------------------------------------------------------------
  initial
    begin : main
      $display("   -- Testbench for sha256 started --");

      init_sim();
      reset_dut();

      if (!$value$plusargs("TESTCASE=%s", test_case))
      begin
         test_case = "fuzz_inputs";
      end
      $display("Testcase: %s", test_case );

      fd = $fopen(test_case, "rb");

      while (!$feof(fd)) begin

        status = $fread(address, fd);
        status = $fread(data, fd);

        address = {{address[07:00]}, {address[15:08]}, {address[23:16]}, {address[31:24]}};
        data = {{data[07:00]}, {data[15:08]}, {data[23:16]}, {data[31:24]}};

        $display("- {opcode: write, addr: %h, data: %h}", address, data);

        write_word(address, data);
      end

      $display("   -- Testbench for sha256 done. --");
      $finish;
    end // main
endmodule // tb_fuzz

//======================================================================
// EOF tb_fuzz.v
//======================================================================
