// Copyright lowRISC contributors.
// Licensed under the Apache License, Version 2.0, see LICENSE for details.
// SPDX-License-Identifier: Apache-2.0
//
// AES core implementation

`include "prim_assert.sv"

module aes_core
  import aes_pkg::*;
  import aes_reg_pkg::*;
#(
  parameter bit          AES192Enable         = 1,
  parameter bit          Masking              = 0,
  parameter sbox_impl_e  SBoxImpl             = SBoxImplLut,
  parameter int unsigned SecStartTriggerDelay = 0,
  parameter bit          SecAllowForcingMasks = 0,

  localparam int         NumShares            = Masking ? 2 : 1, // derived parameter

  parameter logic [WidthPRDClearing-1:0] SeedClearing = DefaultSeedClearing,
  parameter logic  [WidthPRDMasking-1:0] SeedMasking  = DefaultSeedMasking
) (
  input  logic                        clk_i,
  input  logic                        rst_ni,

  // Entropy request interfaces for clearing and masking PRNGs
  output logic                        entropy_clearing_req_o,
  input  logic                        entropy_clearing_ack_i,
  input  logic [WidthPRDClearing-1:0] entropy_clearing_i,
  output logic                        entropy_masking_req_o,
  input  logic                        entropy_masking_ack_i,
  input  logic  [WidthPRDMasking-1:0] entropy_masking_i,

  // Alerts
  output logic                        ctrl_err_update_o,
  output logic                        ctrl_err_storage_o,

  // Bus Interface
  input  aes_reg2hw_t                 reg2hw,
  output aes_hw2reg_t                 hw2reg
);

  // Signals
  logic                        ctrl_re;
  logic                        ctrl_qe;
  logic                        ctrl_we;
  aes_op_e                     aes_op_q;
  aes_mode_e                   mode;
  aes_mode_e                   aes_mode_q;
  ciph_op_e                    cipher_op;
  key_len_e                    key_len;
  key_len_e                    key_len_q;
  logic                        manual_operation_q;
  logic                        force_zero_masks_q;
  ctrl_reg_t                   ctrl_d, ctrl_q;

  logic        [3:0][3:0][7:0] state_in;
  si_sel_e                     state_in_sel;
  logic        [3:0][3:0][7:0] add_state_in;
  add_si_sel_e                 add_state_in_sel;

  logic        [3:0][3:0][7:0] state_mask;
  logic        [3:0][3:0][7:0] state_init [NumShares];
  logic        [3:0][3:0][7:0] state_done [NumShares];
  logic        [3:0][3:0][7:0] state_out;

  logic            [7:0][31:0] key_init [2];
  logic            [7:0]       key_init_qe [2];
  logic            [7:0][31:0] key_init_d [2];
  logic            [7:0][31:0] key_init_q [2];
  logic            [7:0][31:0] key_init_cipher [NumShares];
  logic            [7:0]       key_init_we [2];
  key_init_sel_e               key_init_sel;

  logic            [3:0][31:0] iv;
  logic            [3:0]       iv_qe;
  logic            [7:0][15:0] iv_d;
  logic            [7:0][15:0] iv_q;
  logic            [7:0]       iv_we;
  iv_sel_e                     iv_sel;

  logic            [7:0][15:0] ctr;
  logic            [7:0]       ctr_we;
  logic                        ctr_incr;
  logic                        ctr_ready;

  logic            [3:0][31:0] data_in_prev_d;
  logic            [3:0][31:0] data_in_prev_q;
  logic                        data_in_prev_we;
  dip_sel_e                    data_in_prev_sel;

  logic            [3:0][31:0] data_in;
  logic            [3:0]       data_in_qe;
  logic                        data_in_we;

  logic        [3:0][3:0][7:0] add_state_out;
  add_so_sel_e                 add_state_out_sel;

  logic            [3:0][31:0] data_out_d;
  logic            [3:0][31:0] data_out_q;
  logic                        data_out_we;
  logic                  [3:0] data_out_re;

  logic                        cipher_in_valid;
  logic                        cipher_in_ready;
  logic                        cipher_out_valid;
  logic                        cipher_out_ready;
  /* verilator lint_off UNOPTFLAT */
  logic                        cipher_crypt;
  logic                        cipher_crypt_busy;
  logic                        cipher_dec_key_gen;
  logic                        cipher_dec_key_gen_busy;
  logic                        cipher_key_clear;
  logic                        cipher_key_clear_busy;
  logic                        cipher_data_out_clear;
  logic                        cipher_data_out_clear_busy;

  // Pseudo-random data for clearing purposes
  logic [WidthPRDClearing-1:0] prd_clearing;
  logic                        prd_clearing_upd_req;
  logic                        prd_clearing_upd_ack;
  logic                        prd_clearing_rsd_req;
  logic                        prd_clearing_rsd_ack;
  logic                [127:0] prd_clearing_128;
  logic                [255:0] prd_clearing_256;

  // Unused signals
  logic            [3:0][31:0] unused_data_out_q;
  logic                        unused_force_zero_masks;

  // The clearing PRNG provides pseudo-random data for register clearing purposes.
  aes_prng_clearing #(
    .Width       ( WidthPRDClearing ),
    .DefaultSeed ( SeedClearing     )
  ) u_aes_prng_clearing (
    .clk_i         ( clk_i                  ),
    .rst_ni        ( rst_ni                 ),

    .data_req_i    ( prd_clearing_upd_req   ),
    .data_ack_o    ( prd_clearing_upd_ack   ),
    .data_o        ( prd_clearing           ),
    .reseed_req_i  ( prd_clearing_rsd_req   ),
    .reseed_ack_o  ( prd_clearing_rsd_ack   ),

    .entropy_req_o ( entropy_clearing_req_o ),
    .entropy_ack_i ( entropy_clearing_ack_i ),
    .entropy_i     ( entropy_clearing_i     )
  );

  // Generate clearing signals of appropriate widths.
  localparam int unsigned NumChunks = 128/WidthPRDClearing;
  for (genvar c = 0; c < NumChunks; c++) begin : gen_prd_clearing
    assign prd_clearing_128[c * WidthPRDClearing       +: WidthPRDClearing] = prd_clearing;
    assign prd_clearing_256[c * WidthPRDClearing       +: WidthPRDClearing] = prd_clearing;
    assign prd_clearing_256[c * WidthPRDClearing + 128 +: WidthPRDClearing] = prd_clearing;
  end

  ////////////
  // Inputs //
  ////////////

  always_comb begin : key_init_get
    for (int i=0; i<8; i++) begin
      key_init[0][i]    = reg2hw.key_share0[i].q;
      key_init_qe[0][i] = reg2hw.key_share0[i].qe;
      key_init[1][i]    = reg2hw.key_share1[i].q;
      key_init_qe[1][i] = reg2hw.key_share1[i].qe;
    end
  end

  always_comb begin : iv_get
    for (int i=0; i<4; i++) begin
      iv[i]    = reg2hw.iv[i].q;
      iv_qe[i] = reg2hw.iv[i].qe;
    end
  end

  always_comb begin : data_in_get
    for (int i=0; i<4; i++) begin
      data_in[i]    = reg2hw.data_in[i].q;
      data_in_qe[i] = reg2hw.data_in[i].qe;
    end
  end

  always_comb begin : data_out_get
    for (int i=0; i<4; i++) begin
      // data_out is actually hwo, but we need hrw for hwre
      unused_data_out_q[i] = reg2hw.data_out[i].q;
      data_out_re[i]       = reg2hw.data_out[i].re;
    end
  end

  //////////////////////
  // Key, IV and Data //
  //////////////////////

  // Initial Key registers
  always_comb begin : key_init_mux
    unique case (key_init_sel)
      KEY_INIT_INPUT: key_init_d = key_init;
      KEY_INIT_CLEAR: key_init_d = '{default: prd_clearing_256};
      default:        key_init_d = '{default: prd_clearing_256};
    endcase
  end

  always_ff @(posedge clk_i) begin : key_init_reg
    for (int s=0; s<2; s++) begin
      for (int i=0; i<8; i++) begin
        if (key_init_we[s][i]) begin
          key_init_q[s][i] <= key_init_d[s][i];
        end
      end
    end
  end

  // IV registers
  always_comb begin : iv_mux
    unique case (iv_sel)
      IV_INPUT:        iv_d = iv;
      IV_DATA_OUT:     iv_d = data_out_d;
      IV_DATA_OUT_RAW: iv_d = aes_transpose(state_out);
      IV_DATA_IN_PREV: iv_d = data_in_prev_q;
      IV_CTR:          iv_d = ctr;
      IV_CLEAR:        iv_d = prd_clearing_128;
      default:         iv_d = prd_clearing_128;
    endcase
  end

  always_ff @(posedge clk_i) begin : iv_reg
    for (int i=0; i<8; i++) begin
      if (iv_we[i]) begin
        iv_q[i] <= iv_d[i];
      end
    end
  end

  // Previous input data register
  always_comb begin : data_in_prev_mux
    unique case (data_in_prev_sel)
      DIP_DATA_IN: data_in_prev_d = data_in;
      DIP_CLEAR:   data_in_prev_d = prd_clearing_128;
      default:     data_in_prev_d = prd_clearing_128;
    endcase
  end

  always_ff @(posedge clk_i) begin : data_in_prev_reg
    if (data_in_prev_we) begin
      data_in_prev_q <= data_in_prev_d;
    end
  end

  /////////////
  // Counter //
  /////////////

  aes_ctr u_aes_ctr (
    .clk_i    ( clk_i     ),
    .rst_ni   ( rst_ni    ),

    .incr_i   ( ctr_incr  ),
    .ready_o  ( ctr_ready ),

    .ctr_i    ( iv_q      ),
    .ctr_o    ( ctr       ),
    .ctr_we_o ( ctr_we    )
  );

  /////////////////
  // Cipher Core //
  /////////////////

  // Cipher core operation
  assign cipher_op = (aes_mode_q == AES_ECB && aes_op_q == AES_ENC) ? CIPH_FWD :
                     (aes_mode_q == AES_ECB && aes_op_q == AES_DEC) ? CIPH_INV :
                     (aes_mode_q == AES_CBC && aes_op_q == AES_ENC) ? CIPH_FWD :
                     (aes_mode_q == AES_CBC && aes_op_q == AES_DEC) ? CIPH_INV :
                     (aes_mode_q == AES_CFB)                        ? CIPH_FWD :
                     (aes_mode_q == AES_OFB)                        ? CIPH_FWD :
                     (aes_mode_q == AES_CTR)                        ? CIPH_FWD : CIPH_FWD;

  // Convert input data/IV to state format (every word corresponds to one state column).
  // Mux for state input
  always_comb begin : state_in_mux
    unique case (state_in_sel)
      SI_ZERO: state_in = '0;
      SI_DATA: state_in = aes_transpose(data_in);
      default: state_in = '0;
    endcase
  end

  // Mux for addition to state input
  always_comb begin : add_state_in_mux
    unique case (add_state_in_sel)
      ADD_SI_ZERO: add_state_in = '0;
      ADD_SI_IV:   add_state_in = aes_transpose(iv_q);
      default:     add_state_in = '0;
    endcase
  end

  if (!Masking) begin : gen_state_init_unmasked
    assign state_init[0] = state_in ^ add_state_in;

    logic [3:0][3:0][7:0] unused_state_mask;
    assign unused_state_mask = state_mask;

  end else begin : gen_state_init_masked
    assign state_init[0] = (state_in ^ add_state_in) ^ state_mask; // Masked data share
    assign state_init[1] = state_mask;                             // Mask share
  end

  if (!Masking) begin : gen_key_init_unmasked
    // Combine the two key shares for the unmasked cipher core. This causes SCA leakage of the key
    // and thus should be avoided.
    assign key_init_cipher[0] = key_init_q[0] ^ key_init_q[1];

  end else begin : gen_key_init_masked
    // Forward the masked key share and the mask share to the masked cipher core.
    assign key_init_cipher    = key_init_q;
  end

  // Cipher core
  aes_cipher_core #(
    .AES192Enable         ( AES192Enable         ),
    .Masking              ( Masking              ),
    .SBoxImpl             ( SBoxImpl             ),
    .SecAllowForcingMasks ( SecAllowForcingMasks ),
    .SeedMasking          ( SeedMasking          )
  ) u_aes_cipher_core (
    .clk_i              ( clk_i                      ),
    .rst_ni             ( rst_ni                     ),

    .in_valid_i         ( cipher_in_valid            ),
    .in_ready_o         ( cipher_in_ready            ),

    .out_valid_o        ( cipher_out_valid           ),
    .out_ready_i        ( cipher_out_ready           ),

    .cfg_valid_i        ( ~ctrl_err_storage_o        ),
    .op_i               ( cipher_op                  ),
    .key_len_i          ( key_len_q                  ),
    .crypt_i            ( cipher_crypt               ),
    .crypt_o            ( cipher_crypt_busy          ),
    .dec_key_gen_i      ( cipher_dec_key_gen         ),
    .dec_key_gen_o      ( cipher_dec_key_gen_busy    ),
    .key_clear_i        ( cipher_key_clear           ),
    .key_clear_o        ( cipher_key_clear_busy      ),
    .data_out_clear_i   ( cipher_data_out_clear      ),
    .data_out_clear_o   ( cipher_data_out_clear_busy ),

    .prd_clearing_i     ( prd_clearing               ),

    .force_zero_masks_i ( force_zero_masks_q         ),
    .data_in_mask_o     ( state_mask                 ),
    .entropy_req_o      ( entropy_masking_req_o      ),
    .entropy_ack_i      ( entropy_masking_ack_i      ),
    .entropy_i          ( entropy_masking_i          ),

    .state_init_i       ( state_init                 ),
    .key_init_i         ( key_init_cipher            ),
    .state_o            ( state_done                 )
  );

  if (!Masking) begin : gen_state_out_unmasked
    assign state_out = state_done[0];
  end else begin : gen_state_out_masked
    // Unmask the cipher core output. This causes SCA leakage and should thus be avoided. This will
    // be reworked in the future when masking the counter and feedback path through the IV regs.
    assign state_out = state_done[0] ^ state_done[1];
  end

  // Mux for addition to state output
  always_comb begin : add_state_out_mux
    unique case (add_state_out_sel)
      ADD_SO_ZERO: add_state_out = '0;
      ADD_SO_IV:   add_state_out = aes_transpose(iv_q);
      ADD_SO_DIP:  add_state_out = aes_transpose(data_in_prev_q);
      default:     add_state_out = '0;
    endcase
  end

  // Convert output state to output data format (every column corresponds to one output word).
  assign data_out_d = aes_transpose(state_out ^ add_state_out);

  //////////////////////
  // Control Register //
  //////////////////////

  // Get and resolve values from register interface.
  assign ctrl_d.operation = aes_op_e'(reg2hw.ctrl_shadowed.operation.q);

  assign mode = aes_mode_e'(reg2hw.ctrl_shadowed.mode.q);
  always_comb begin : mode_get
    unique case (mode)
      AES_ECB: ctrl_d.mode = AES_ECB;
      AES_CBC: ctrl_d.mode = AES_CBC;
      AES_CFB: ctrl_d.mode = AES_CFB;
      AES_OFB: ctrl_d.mode = AES_OFB;
      AES_CTR: ctrl_d.mode = AES_CTR;
      default: ctrl_d.mode = AES_NONE; // unsupported values are mapped to AES_NONE
    endcase
  end

  assign key_len = key_len_e'(reg2hw.ctrl_shadowed.key_len.q);
  always_comb begin : key_len_get
    unique case (key_len)
      AES_128: ctrl_d.key_len = AES_128;
      AES_256: ctrl_d.key_len = AES_256;
      AES_192: ctrl_d.key_len = AES192Enable ? AES_192 : AES_256;
      default: ctrl_d.key_len = AES_256; // unsupported values are mapped to AES_256
    endcase
  end

  assign ctrl_d.manual_operation = reg2hw.ctrl_shadowed.manual_operation.q;

  // SecAllowForcingMasks forbids forcing the masks. Forcing the masks to zero is only
  // useful for SCA.
  assign ctrl_d.force_zero_masks = SecAllowForcingMasks ?
      reg2hw.ctrl_shadowed.force_zero_masks.q : 1'b0;
  assign unused_force_zero_masks = SecAllowForcingMasks ?
      1'b0 : reg2hw.ctrl_shadowed.force_zero_masks.q;

  // Get and forward write enable. Writes are only allowed if the module is idle.
  assign ctrl_re = reg2hw.ctrl_shadowed.operation.re & reg2hw.ctrl_shadowed.mode.re &
      reg2hw.ctrl_shadowed.key_len.re & reg2hw.ctrl_shadowed.manual_operation.re &
      reg2hw.ctrl_shadowed.force_zero_masks.re;
  assign ctrl_qe = reg2hw.ctrl_shadowed.operation.qe & reg2hw.ctrl_shadowed.mode.qe &
      reg2hw.ctrl_shadowed.key_len.qe & reg2hw.ctrl_shadowed.manual_operation.qe &
      reg2hw.ctrl_shadowed.force_zero_masks.qe;

  // Shadowed register primitve
  prim_subreg_shadow #(
    .DW       ( $bits(ctrl_reg_t) ),
    .SWACCESS ( "WO"              ),
    .RESVAL   ( CTRL_RESET        )
  ) u_ctrl_reg_shadowed (
    .clk_i       ( clk_i              ),
    .rst_ni      ( rst_ni             ),
    .re          ( ctrl_re            ),
    .we          ( ctrl_we            ),
    .wd          ( ctrl_d             ),
    .de          ( 1'b0               ),
    .d           ( '0                 ),
    .qe          (                    ),
    .q           ( ctrl_q             ),
    .qs          (                    ),
    .err_update  ( ctrl_err_update_o  ),
    .err_storage ( ctrl_err_storage_o )
  );

  // Make sure the storage error is observable via status register.
  assign hw2reg.status.ctrl_err_storage.d  = ctrl_err_storage_o;
  assign hw2reg.status.ctrl_err_storage.de = ctrl_err_storage_o;

  // Get shorter references.
  assign aes_op_q           = ctrl_q.operation;
  assign aes_mode_q         = ctrl_q.mode;
  assign key_len_q          = ctrl_q.key_len;
  assign manual_operation_q = ctrl_q.manual_operation;
  assign force_zero_masks_q = ctrl_q.force_zero_masks;

  // Unused alert signals
  logic unused_alert_signals;
  assign unused_alert_signals = ^reg2hw.alert_test;

  /////////////
  // Control //
  /////////////

  // Control
  aes_control #(
    .SecStartTriggerDelay ( SecStartTriggerDelay )
  ) u_aes_control (
    .clk_i                   ( clk_i                            ),
    .rst_ni                  ( rst_ni                           ),

    .ctrl_qe_i               ( ctrl_qe                          ),
    .ctrl_we_o               ( ctrl_we                          ),
    .ctrl_err_storage_i      ( ctrl_err_storage_o               ),
    .op_i                    ( aes_op_q                         ),
    .mode_i                  ( aes_mode_q                       ),
    .cipher_op_i             ( cipher_op                        ),
    .manual_operation_i      ( manual_operation_q               ),
    .start_i                 ( reg2hw.trigger.start.q           ),
    .key_clear_i             ( reg2hw.trigger.key_clear.q       ),
    .iv_clear_i              ( reg2hw.trigger.iv_clear.q        ),
    .data_in_clear_i         ( reg2hw.trigger.data_in_clear.q   ),
    .data_out_clear_i        ( reg2hw.trigger.data_out_clear.q  ),
    .prng_reseed_i           ( reg2hw.trigger.prng_reseed.q     ),

    .key_init_qe_i           ( key_init_qe                      ),
    .iv_qe_i                 ( iv_qe                            ),
    .data_in_qe_i            ( data_in_qe                       ),
    .data_out_re_i           ( data_out_re                      ),
    .data_in_we_o            ( data_in_we                       ),
    .data_out_we_o           ( data_out_we                      ),

    .data_in_prev_sel_o      ( data_in_prev_sel                 ),
    .data_in_prev_we_o       ( data_in_prev_we                  ),

    .state_in_sel_o          ( state_in_sel                     ),
    .add_state_in_sel_o      ( add_state_in_sel                 ),
    .add_state_out_sel_o     ( add_state_out_sel                ),

    .ctr_incr_o              ( ctr_incr                         ),
    .ctr_ready_i             ( ctr_ready                        ),
    .ctr_we_i                ( ctr_we                           ),

    .cipher_in_valid_o       ( cipher_in_valid                  ),
    .cipher_in_ready_i       ( cipher_in_ready                  ),
    .cipher_out_valid_i      ( cipher_out_valid                 ),
    .cipher_out_ready_o      ( cipher_out_ready                 ),
    .cipher_crypt_o          ( cipher_crypt                     ),
    .cipher_crypt_i          ( cipher_crypt_busy                ),
    .cipher_dec_key_gen_o    ( cipher_dec_key_gen               ),
    .cipher_dec_key_gen_i    ( cipher_dec_key_gen_busy          ),
    .cipher_key_clear_o      ( cipher_key_clear                 ),
    .cipher_key_clear_i      ( cipher_key_clear_busy            ),
    .cipher_data_out_clear_o ( cipher_data_out_clear            ),
    .cipher_data_out_clear_i ( cipher_data_out_clear_busy       ),

    .key_init_sel_o          ( key_init_sel                     ),
    .key_init_we_o           ( key_init_we                      ),
    .iv_sel_o                ( iv_sel                           ),
    .iv_we_o                 ( iv_we                            ),

    .prng_data_req_o         ( prd_clearing_upd_req             ),
    .prng_data_ack_i         ( prd_clearing_upd_ack             ),
    .prng_reseed_req_o       ( prd_clearing_rsd_req             ),
    .prng_reseed_ack_i       ( prd_clearing_rsd_ack             ),

    .start_o                 ( hw2reg.trigger.start.d           ),
    .start_we_o              ( hw2reg.trigger.start.de          ),
    .key_clear_o             ( hw2reg.trigger.key_clear.d       ),
    .key_clear_we_o          ( hw2reg.trigger.key_clear.de      ),
    .iv_clear_o              ( hw2reg.trigger.iv_clear.d        ),
    .iv_clear_we_o           ( hw2reg.trigger.iv_clear.de       ),
    .data_in_clear_o         ( hw2reg.trigger.data_in_clear.d   ),
    .data_in_clear_we_o      ( hw2reg.trigger.data_in_clear.de  ),
    .data_out_clear_o        ( hw2reg.trigger.data_out_clear.d  ),
    .data_out_clear_we_o     ( hw2reg.trigger.data_out_clear.de ),
    .prng_reseed_o           ( hw2reg.trigger.prng_reseed.d     ),
    .prng_reseed_we_o        ( hw2reg.trigger.prng_reseed.de    ),

    .output_valid_o          ( hw2reg.status.output_valid.d     ),
    .output_valid_we_o       ( hw2reg.status.output_valid.de    ),
    .input_ready_o           ( hw2reg.status.input_ready.d      ),
    .input_ready_we_o        ( hw2reg.status.input_ready.de     ),
    .idle_o                  ( hw2reg.status.idle.d             ),
    .idle_we_o               ( hw2reg.status.idle.de            ),
    .stall_o                 ( hw2reg.status.stall.d            ),
    .stall_we_o              ( hw2reg.status.stall.de           )
  );

  // Input data register clear
  always_comb begin : data_in_reg_clear
    for (int i=0; i<4; i++) begin
      hw2reg.data_in[i].d  = '0;
      hw2reg.data_in[i].de = data_in_we;
    end
  end

  /////////////
  // Outputs //
  /////////////

  always_ff @(posedge clk_i) begin : data_out_reg
    if (data_out_we) begin
      data_out_q <= data_out_d;
    end
  end

  always_comb begin : key_reg_put
    for (int i=0; i<8; i++) begin
      hw2reg.key_share0[i].d = key_init_q[0][i];
      hw2reg.key_share1[i].d = key_init_q[1][i];
    end
  end

  always_comb begin : iv_reg_put
    for (int i=0; i<4; i++) begin
      hw2reg.iv[i].d  = {iv_q[2*i+1], iv_q[2*i]};
    end
  end

  always_comb begin : data_out_put
    for (int i=0; i<4; i++) begin
      hw2reg.data_out[i].d = data_out_q[i];
    end
  end

  assign hw2reg.ctrl_shadowed.mode.d    = {aes_mode_q};
  assign hw2reg.ctrl_shadowed.key_len.d = {key_len_q};

  // These fields are actually hro. But software must be able observe the current value (rw).
  assign hw2reg.ctrl_shadowed.operation.d        = {aes_op_q};
  assign hw2reg.ctrl_shadowed.manual_operation.d = manual_operation_q;
  assign hw2reg.ctrl_shadowed.force_zero_masks.d = force_zero_masks_q;

  ////////////////
  // Assertions //
  ////////////////

  // Selectors must be known/valid
  `ASSERT_KNOWN(AesKeyInitSelKnown, key_init_sel)
  `ASSERT(AesIvSelValid, iv_sel inside {
      IV_INPUT,
      IV_DATA_OUT,
      IV_DATA_OUT_RAW,
      IV_DATA_IN_PREV,
      IV_CTR,
      IV_CLEAR
      })
  `ASSERT_KNOWN(AesDataInPrevSelKnown, data_in_prev_sel)
  `ASSERT(AesModeValid, !ctrl_err_storage_o |-> aes_mode_q inside {
      AES_ECB,
      AES_CBC,
      AES_CFB,
      AES_OFB,
      AES_CTR,
      AES_NONE
      })
  `ASSERT_KNOWN(AesOpKnown, aes_op_q)
  `ASSERT_KNOWN(AesStateInSelKnown, state_in_sel)
  `ASSERT_KNOWN(AesAddStateInSelKnown, add_state_in_sel)
  `ASSERT(AesAddStateOutSelValid, add_state_out_sel inside {
      ADD_SO_ZERO,
      ADD_SO_IV,
      ADD_SO_DIP
      })

endmodule
