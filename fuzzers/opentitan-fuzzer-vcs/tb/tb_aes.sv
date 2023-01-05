// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

`include "prim_assert.sv"

module tb_aes();

  parameter CLK_HALF_PERIOD = 2;
  parameter CLK_PERIOD = 2 * CLK_HALF_PERIOD;
  parameter             WIDTH = 32;

  parameter KREAD = 2;
  parameter KWRITE = 3;
  parameter KWAIT = 1;

  import aes_pkg::*;
  import aes_reg_pkg::*;
  import tlul_pkg::*;
  // import prim_alert_pkg::*;

  string test_case;
  integer size;
  logic instr_valid;

  logic [8-1:0]opcode;
  logic [32-1:0]address;
  logic [WIDTH-1:0]data;
  logic [31:0] rdata;

  int fd;
  int status;

  reg clk, rst_n; 
  wire idle,alert_tx;
  reg [7:0] alert_rx;

    reg [3:0] a;
    reg [3:0] b;

  // dut signals
  // prim_alert_pkg::alert_rx_t [NumAlerts-1:0] alert_rx;
  // prim_alert_pkg::alert_tx_t [NumAlerts-1:0] alert_tx;

  tlul_pkg::tl_h2d_t tl_i = tlul_pkg::TL_H2D_DEFAULT;
  tlul_pkg::tl_d2h_t tl_o;

  aes dut (
    .clk_i  (clk),
    .rst_ni (rst_n),

    .idle_o (idle),

    .tl_i   (tl_i),
    .tl_o   (tl_o),

    .alert_rx_i (alert_rx),
    .alert_tx_o (alert_tx)
  );

  //----------------------------------------------------------------
  // clk_gen
  //
  // Clock generator process.
  //----------------------------------------------------------------
  always
    begin : clk_gen
      #CLK_HALF_PERIOD clk = !clk;
    end // clk_gen

  //----------------------------------------------------------------
  // reset_dut()
  //
  // Toggles reset to force the DUT into a well defined state.
  //----------------------------------------------------------------
  task reset_dut;
    begin
      $display("*** Toggle reset.");
      rst_n = 0;
      alert_rx = 8'b0;
      tl_i.a_valid   = 1'b 0;
      tl_i.a_address = 32'b0;
      tl_i.a_opcode  = 3'b0;
      tl_i.a_data    = 32'b0;
      tl_i.a_mask    = 4'b0;
      tl_i.a_param   = 3'b0;
      tl_i.a_size    = 2'b0;
      tl_i.a_source  = 8'b0;
      tl_i.a_user    = 16'b0;
      tl_i.d_ready   = 1'b 0;
      #(100 * CLK_PERIOD);
      rst_n = 1;
      tl_i.d_ready   = 1'b1;
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
      clk = 0;
      rst_n = 0;
    end
  endtask // init_dut
  
  //----------------------------------------------------------------
  // error()
  //
  // handle error
  //----------------------------------------------------------------
  task error;
    begin
      $display("Tesbench ended earlier than expected");
      $finish;
    end
  endtask // init_dut

    /////////////////
  // TL-UL agent //
  /////////////////
  task automatic tlul_write(
    const ref logic clk,

    ref tlul_pkg::tl_h2d_t       h2d,
    const ref tlul_pkg::tl_d2h_t d2h,

    input logic [31:0] address,
    input logic [31:0] wdata,
    input logic [ 3:0] wstrb
  );

    // Assume always called this task @(posedge clk);
    h2d.a_address = address;
    h2d.a_opcode  = tlul_pkg::PutFullData;
    h2d.a_data    = wdata;
    h2d.a_mask    = wstrb;
    h2d.a_param   = '0;
    h2d.a_size    = $clog2($countones(wstrb));
    h2d.a_source  = '0;
    h2d.d_ready   = 1'b0;

    @(negedge clk);
    h2d.a_valid   = 1'b1;

    // Due to interity checker instance, `a_ready` from SW view is delayed.
    // Need to see the a_ready then wait posedge.
    // Previously: @(posedge clk iff d2h.a_ready);
    wait(d2h.a_ready);
    @(negedge clk);
    h2d.a_valid = 1'b0;
    h2d.d_ready = 1'b1;
    wait(d2h.d_valid);
    @(negedge clk);
    h2d.d_ready = 1'b1;
    // @(posedge clk);
    // h2d.d_ready = 1'b0;
    @(posedge clk);

  endtask : tlul_write

  task automatic tlul_read(
    const ref logic clk,

    ref tlul_pkg::tl_h2d_t       h2d,
    const ref tlul_pkg::tl_d2h_t d2h,

    input  logic [31:0] address,
    output logic [31:0] rdata
  );

    // Assume always called this task @(posedge clk);
    h2d.a_valid   = 1'b1;
    h2d.a_address = address;
    h2d.a_opcode  = tlul_pkg::Get;
    h2d.a_data    = '0;
    h2d.a_mask    = '1;
    h2d.a_param   = '0;
    h2d.a_size    = 2;
    h2d.a_source  = '0;
    h2d.d_ready   = 1'b0;

    @(posedge clk);

    wait(d2h.a_ready);
    @(posedge clk);
    h2d.a_valid = 1'b 0;
    h2d.d_ready = 1'b 1;
    wait(d2h.d_valid);
    @(posedge clk);
    rdata       = d2h.d_data;
    h2d.d_ready = 1'b1;
    @(posedge clk);

  endtask : tlul_read


// map fuzzing inputs to the tile link bus
initial
begin : main
  
  init_sim();

  $fsdbDumpfile("trace.fsdb");
  $dumpvars;
  $fsdbDumpvars(0, dut);
  $dumpon();

        a = 4'b0000;
        b = ~|a;


  $display("    --  Testbench for aes started -- %b", b);

  reset_dut();

    if (!$value$plusargs("TESTCASE=%s", test_case))
    begin
       test_case = "fuzz_inputs";
    end
    $display("Testcase: %s", test_case );

    fd = $fopen(test_case, "rb");

    instr_valid = 1;

    while (instr_valid == 1 || !$feof(fd)) begin
    // repeat (10) begin
      // status = $fscanf(fd, "%c",opcode);
      // if(status != 1) error();
      status = $fread(opcode, fd);

      case(opcode)
        KWAIT: begin
          $display("- {opcode: wait, addr: 0, data: 0}");
          #(4 * CLK_HALF_PERIOD);
        end

        KREAD: begin
          status = $fread(address, fd);
          
          address = {{address[07:00]}, {address[15:08]}, {address[23:16]}, {address[31:24]}};

          @(posedge clk);
          tlul_read(clk, tl_i, tl_o, address, rdata);
          $display("- {opcode: read, addr: %h, data: %h}", address, rdata);

          if(tl_o.d_error == 1'b1) begin
            $finish;
          end
        end

        KWRITE: begin
          status = $fread(address, fd);
          status = $fread(data, fd);

          address = {{address[07:00]}, {address[15:08]}, {address[23:16]}, {address[31:24]}};
          data = {{data[07:00]}, {data[15:08]}, {data[23:16]}, {data[31:24]}};

          $display("- {opcode: write, addr: %h, data: %h}", address, data);
          // @(posedge clk);
          tlul_write(clk, tl_i, tl_o, address, data, 4'b 1111);

          if(tl_o.d_error == 1'b1) begin
            $finish;
          end

        end
        default: begin
          $display("- unknown {opcode: %d, addr: 0, data: 0}", opcode);
          opcode = 8'h0;
          instr_valid = 0;
          $fclose(fd);
          $display("   -- Testbench for lowrisc::ip::aes done. --");
          $finish;
        end
      endcase

    end
    
    $fclose(fd);

    $display("   -- Testbench for lowrisc::ip::aes done. --");
    $finish;
end

endmodule
