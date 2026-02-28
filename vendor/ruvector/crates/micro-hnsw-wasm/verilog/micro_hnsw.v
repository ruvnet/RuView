// Micro HNSW - ASIC Hardware Description
// Ultra-minimal HNSW accelerator for vector similarity search
//
// Design specifications:
// - Fixed-point arithmetic (Q8.8 format)
// - 256 max vectors, 64 dimensions
// - 8 neighbors per node, 4 levels
// - Pipelined distance computation
// - AXI-Lite interface for host communication
//
// Target: ASIC synthesis with <50K gates

`timescale 1ns / 1ps

module micro_hnsw #(
    parameter MAX_VECTORS = 256,
    parameter MAX_DIMS = 64,
    parameter MAX_NEIGHBORS = 8,
    parameter MAX_LEVELS = 4,
    parameter DATA_WIDTH = 16,      // Q8.8 fixed-point
    parameter ADDR_WIDTH = 8        // log2(MAX_VECTORS)
)(
    input  wire                     clk,
    input  wire                     rst_n,

    // Control interface
    input  wire                     cmd_valid,
    output reg                      cmd_ready,
    input  wire [2:0]               cmd_op,         // 0=NOP, 1=INIT, 2=INSERT, 3=SEARCH
    input  wire [7:0]               cmd_dims,
    input  wire [7:0]               cmd_k,

    // Vector data interface
    input  wire                     vec_valid,
    output wire                     vec_ready,
    input  wire [DATA_WIDTH-1:0]    vec_data,
    input  wire                     vec_last,

    // Result interface
    output reg                      result_valid,
    input  wire                     result_ready,
    output reg  [ADDR_WIDTH-1:0]    result_idx,
    output reg  [DATA_WIDTH-1:0]    result_dist,
    output reg                      result_last,

    // Status
    output reg  [ADDR_WIDTH-1:0]    vector_count
);

// ============ Local Parameters ============
localparam STATE_IDLE       = 3'd0;
localparam STATE_LOAD_VEC   = 3'd1;
localparam STATE_COMPUTE    = 3'd2;
localparam STATE_SEARCH     = 3'd3;
localparam STATE_OUTPUT     = 3'd4;

// ============ Memories ============
// Vector storage (256 x 64 x 16-bit = 256KB)
reg [DATA_WIDTH-1:0] vectors [0:MAX_VECTORS-1][0:MAX_DIMS-1];

// Graph structure - neighbor lists
reg [ADDR_WIDTH-1:0] neighbors [0:MAX_VECTORS-1][0:MAX_LEVELS-1][0:MAX_NEIGHBORS-1];
reg [3:0] neighbor_count [0:MAX_VECTORS-1][0:MAX_LEVELS-1];
reg [1:0] node_level [0:MAX_VECTORS-1];

// ============ Registers ============
reg [2:0] state;
reg [ADDR_WIDTH-1:0] entry_point;
reg [1:0] max_level;
reg [7:0] current_dims;

// Vector loading
reg [DATA_WIDTH-1:0] query_buf [0:MAX_DIMS-1];
reg [DATA_WIDTH-1:0] insert_buf [0:MAX_DIMS-1];
reg [5:0] load_idx;

// Search state
reg [ADDR_WIDTH-1:0] current_node;
reg [1:0] current_level;
reg [7:0] current_k;
reg [3:0] neighbor_idx;

// Candidate buffer (sorted by distance)
reg [ADDR_WIDTH-1:0] candidates [0:15];
reg [DATA_WIDTH-1:0] cand_dist [0:15];
reg [3:0] cand_count;

// Distance computation
reg [31:0] dist_accum;
reg [5:0] dist_dim;
reg dist_computing;
reg [ADDR_WIDTH-1:0] dist_target;

// Visited flags (bit vector)
reg [MAX_VECTORS-1:0] visited;

// ============ Vector Ready ============
assign vec_ready = (state == STATE_LOAD_VEC);

// ============ State Machine ============
always @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        state <= STATE_IDLE;
        cmd_ready <= 1'b1;
        result_valid <= 1'b0;
        vector_count <= 0;
        entry_point <= 0;
        max_level <= 0;
        current_dims <= 32;
    end else begin
        case (state)
            STATE_IDLE: begin
                result_valid <= 1'b0;
                if (cmd_valid && cmd_ready) begin
                    cmd_ready <= 1'b0;
                    case (cmd_op)
                        3'd1: begin // INIT
                            current_dims <= cmd_dims;
                            vector_count <= 0;
                            entry_point <= 0;
                            max_level <= 0;
                            cmd_ready <= 1'b1;
                        end
                        3'd2: begin // INSERT
                            load_idx <= 0;
                            state <= STATE_LOAD_VEC;
                        end
                        3'd3: begin // SEARCH
                            load_idx <= 0;
                            current_k <= cmd_k;
                            state <= STATE_LOAD_VEC;
                        end
                        default: cmd_ready <= 1'b1;
                    endcase
                end
            end

            STATE_LOAD_VEC: begin
                if (vec_valid) begin
                    if (cmd_op == 3'd2) begin
                        insert_buf[load_idx] <= vec_data;
                    end else begin
                        query_buf[load_idx] <= vec_data;
                    end

                    if (vec_last || load_idx == current_dims - 1) begin
                        if (cmd_op == 3'd2) begin
                            state <= STATE_COMPUTE; // Insert processing
                        end else begin
                            state <= STATE_SEARCH;  // Search processing
                        end
                    end else begin
                        load_idx <= load_idx + 1;
                    end
                end
            end

            STATE_COMPUTE: begin
                // Store vector
                integer i;
                for (i = 0; i < MAX_DIMS; i = i + 1) begin
                    vectors[vector_count][i] <= insert_buf[i];
                end

                // Generate random level (simplified)
                node_level[vector_count] <= vector_count[1:0] & 2'b11;

                // Initialize neighbors
                for (i = 0; i < MAX_LEVELS; i = i + 1) begin
                    neighbor_count[vector_count][i] <= 0;
                end

                // Update entry point for first vector
                if (vector_count == 0) begin
                    entry_point <= 0;
                    max_level <= 0;
                end else begin
                    // Simple nearest neighbor connection (level 0 only for minimal design)
                    if (neighbor_count[vector_count][0] < MAX_NEIGHBORS) begin
                        // Connect to entry point
                        neighbors[vector_count][0][0] <= entry_point;
                        neighbor_count[vector_count][0] <= 1;

                        // Bidirectional connection
                        if (neighbor_count[entry_point][0] < MAX_NEIGHBORS) begin
                            neighbors[entry_point][0][neighbor_count[entry_point][0]] <= vector_count;
                            neighbor_count[entry_point][0] <= neighbor_count[entry_point][0] + 1;
                        end
                    end
                end

                vector_count <= vector_count + 1;
                cmd_ready <= 1'b1;
                state <= STATE_IDLE;
            end

            STATE_SEARCH: begin
                // Initialize search
                visited <= 0;
                cand_count <= 0;
                current_node <= entry_point;
                current_level <= max_level;

                // Start distance computation for entry point
                dist_target <= entry_point;
                dist_accum <= 0;
                dist_dim <= 0;
                dist_computing <= 1'b1;

                // Simple greedy search (one level)
                if (!dist_computing && cand_count < current_k) begin
                    // Add current to candidates
                    candidates[cand_count] <= current_node;
                    cand_dist[cand_count] <= dist_accum[DATA_WIDTH-1:0];
                    cand_count <= cand_count + 1;
                    visited[current_node] <= 1'b1;

                    // Check neighbors
                    if (neighbor_idx < neighbor_count[current_node][0]) begin
                        current_node <= neighbors[current_node][0][neighbor_idx];
                        neighbor_idx <= neighbor_idx + 1;
                        dist_target <= neighbors[current_node][0][neighbor_idx];
                        dist_accum <= 0;
                        dist_dim <= 0;
                        dist_computing <= 1'b1;
                    end else begin
                        state <= STATE_OUTPUT;
                    end
                end
            end

            STATE_OUTPUT: begin
                if (result_ready || !result_valid) begin
                    if (cand_count > 0) begin
                        result_valid <= 1'b1;
                        result_idx <= candidates[0];
                        result_dist <= cand_dist[0];
                        result_last <= (cand_count == 1);

                        // Shift candidates
                        integer j;
                        for (j = 0; j < 15; j = j + 1) begin
                            candidates[j] <= candidates[j+1];
                            cand_dist[j] <= cand_dist[j+1];
                        end
                        cand_count <= cand_count - 1;
                    end else begin
                        result_valid <= 1'b0;
                        cmd_ready <= 1'b1;
                        state <= STATE_IDLE;
                    end
                end
            end
        endcase
    end
end

// ============ Distance Computation Pipeline ============
always @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        dist_computing <= 1'b0;
        dist_accum <= 0;
    end else if (dist_computing) begin
        if (dist_dim < current_dims) begin
            // Compute (query - vector)^2 in fixed-point
            reg signed [DATA_WIDTH:0] diff;
            reg [31:0] sq;

            diff = $signed(query_buf[dist_dim]) - $signed(vectors[dist_target][dist_dim]);
            sq = diff * diff;
            dist_accum <= dist_accum + sq;
            dist_dim <= dist_dim + 1;
        end else begin
            dist_computing <= 1'b0;
        end
    end
end

endmodule


// ============ Distance Unit - Pipelined L2 ============
module distance_unit #(
    parameter DATA_WIDTH = 16,
    parameter MAX_DIMS = 64
)(
    input  wire                     clk,
    input  wire                     rst_n,
    input  wire                     start,
    input  wire [5:0]               dims,
    input  wire [DATA_WIDTH-1:0]    a_data,
    input  wire [DATA_WIDTH-1:0]    b_data,
    output reg  [31:0]              distance,
    output reg                      done
);

reg [5:0] dim_idx;
reg [31:0] accum;
reg computing;

always @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
        done <= 1'b0;
        computing <= 1'b0;
        accum <= 0;
    end else begin
        if (start && !computing) begin
            computing <= 1'b1;
            dim_idx <= 0;
            accum <= 0;
            done <= 1'b0;
        end else if (computing) begin
            if (dim_idx < dims) begin
                // Compute squared difference
                reg signed [DATA_WIDTH:0] diff;
                diff = $signed(a_data) - $signed(b_data);
                accum <= accum + (diff * diff);
                dim_idx <= dim_idx + 1;
            end else begin
                distance <= accum;
                done <= 1'b1;
                computing <= 1'b0;
            end
        end else begin
            done <= 1'b0;
        end
    end
end

endmodule


// ============ Priority Queue for Candidates ============
module priority_queue #(
    parameter DEPTH = 16,
    parameter IDX_WIDTH = 8,
    parameter DIST_WIDTH = 16
)(
    input  wire                     clk,
    input  wire                     rst_n,
    input  wire                     clear,

    // Insert interface
    input  wire                     insert_valid,
    output wire                     insert_ready,
    input  wire [IDX_WIDTH-1:0]     insert_idx,
    input  wire [DIST_WIDTH-1:0]    insert_dist,

    // Pop interface (returns min distance)
    input  wire                     pop_valid,
    output reg                      pop_ready,
    output reg  [IDX_WIDTH-1:0]     pop_idx,
    output reg  [DIST_WIDTH-1:0]    pop_dist,

    // Status
    output reg  [4:0]               count,
    output wire                     empty,
    output wire                     full
);

reg [IDX_WIDTH-1:0] indices [0:DEPTH-1];
reg [DIST_WIDTH-1:0] distances [0:DEPTH-1];

assign empty = (count == 0);
assign full = (count == DEPTH);
assign insert_ready = !full;

integer i;

always @(posedge clk or negedge rst_n) begin
    if (!rst_n || clear) begin
        count <= 0;
        pop_ready <= 1'b0;
    end else begin
        // Insert operation (sorted insert)
        if (insert_valid && !full) begin
            // Find insertion position
            reg [4:0] pos;
            pos = count;

            for (i = count - 1; i >= 0; i = i - 1) begin
                if (insert_dist < distances[i]) begin
                    indices[i+1] <= indices[i];
                    distances[i+1] <= distances[i];
                    pos = i;
                end
            end

            indices[pos] <= insert_idx;
            distances[pos] <= insert_dist;
            count <= count + 1;
        end

        // Pop operation
        if (pop_valid && !empty) begin
            pop_idx <= indices[0];
            pop_dist <= distances[0];
            pop_ready <= 1'b1;

            // Shift elements
            for (i = 0; i < DEPTH - 1; i = i + 1) begin
                indices[i] <= indices[i+1];
                distances[i] <= distances[i+1];
            end
            count <= count - 1;
        end else begin
            pop_ready <= 1'b0;
        end
    end
end

endmodule


// ============ AXI-Lite Wrapper ============
module micro_hnsw_axi #(
    parameter C_S_AXI_DATA_WIDTH = 32,
    parameter C_S_AXI_ADDR_WIDTH = 8
)(
    // AXI-Lite interface
    input  wire                                 S_AXI_ACLK,
    input  wire                                 S_AXI_ARESETN,

    // Write address channel
    input  wire [C_S_AXI_ADDR_WIDTH-1:0]        S_AXI_AWADDR,
    input  wire                                 S_AXI_AWVALID,
    output wire                                 S_AXI_AWREADY,

    // Write data channel
    input  wire [C_S_AXI_DATA_WIDTH-1:0]        S_AXI_WDATA,
    input  wire [(C_S_AXI_DATA_WIDTH/8)-1:0]    S_AXI_WSTRB,
    input  wire                                 S_AXI_WVALID,
    output wire                                 S_AXI_WREADY,

    // Write response channel
    output wire [1:0]                           S_AXI_BRESP,
    output wire                                 S_AXI_BVALID,
    input  wire                                 S_AXI_BREADY,

    // Read address channel
    input  wire [C_S_AXI_ADDR_WIDTH-1:0]        S_AXI_ARADDR,
    input  wire                                 S_AXI_ARVALID,
    output wire                                 S_AXI_ARREADY,

    // Read data channel
    output wire [C_S_AXI_DATA_WIDTH-1:0]        S_AXI_RDATA,
    output wire [1:0]                           S_AXI_RRESP,
    output wire                                 S_AXI_RVALID,
    input  wire                                 S_AXI_RREADY
);

// Register map:
// 0x00: Control (W) - [2:0] cmd_op, [15:8] dims, [23:16] k
// 0x04: Status (R) - [0] ready, [15:8] vector_count
// 0x08: Vector Data (W) - write vector data
// 0x0C: Result (R) - [7:0] idx, [23:8] distance, [31] last

// Internal signals
wire cmd_valid, cmd_ready;
reg [2:0] cmd_op;
reg [7:0] cmd_dims, cmd_k;
wire vec_valid, vec_ready;
reg [15:0] vec_data;
reg vec_last;
wire result_valid, result_ready;
wire [7:0] result_idx;
wire [15:0] result_dist;
wire result_last;
wire [7:0] vector_count;

// Instantiate core
micro_hnsw core (
    .clk(S_AXI_ACLK),
    .rst_n(S_AXI_ARESETN),
    .cmd_valid(cmd_valid),
    .cmd_ready(cmd_ready),
    .cmd_op(cmd_op),
    .cmd_dims(cmd_dims),
    .cmd_k(cmd_k),
    .vec_valid(vec_valid),
    .vec_ready(vec_ready),
    .vec_data(vec_data),
    .vec_last(vec_last),
    .result_valid(result_valid),
    .result_ready(result_ready),
    .result_idx(result_idx),
    .result_dist(result_dist),
    .result_last(result_last),
    .vector_count(vector_count)
);

// AXI-Lite state machine (simplified)
reg aw_ready, w_ready, ar_ready;
reg [1:0] b_resp;
reg b_valid, r_valid;
reg [C_S_AXI_DATA_WIDTH-1:0] r_data;

assign S_AXI_AWREADY = aw_ready;
assign S_AXI_WREADY = w_ready;
assign S_AXI_BRESP = b_resp;
assign S_AXI_BVALID = b_valid;
assign S_AXI_ARREADY = ar_ready;
assign S_AXI_RDATA = r_data;
assign S_AXI_RRESP = 2'b00;
assign S_AXI_RVALID = r_valid;

assign cmd_valid = S_AXI_WVALID && (S_AXI_AWADDR == 8'h00);
assign vec_valid = S_AXI_WVALID && (S_AXI_AWADDR == 8'h08);
assign result_ready = S_AXI_RREADY && (S_AXI_ARADDR == 8'h0C);

always @(posedge S_AXI_ACLK or negedge S_AXI_ARESETN) begin
    if (!S_AXI_ARESETN) begin
        aw_ready <= 1'b1;
        w_ready <= 1'b1;
        ar_ready <= 1'b1;
        b_valid <= 1'b0;
        r_valid <= 1'b0;
    end else begin
        // Write handling
        if (S_AXI_AWVALID && S_AXI_WVALID && aw_ready && w_ready) begin
            case (S_AXI_AWADDR)
                8'h00: begin
                    cmd_op <= S_AXI_WDATA[2:0];
                    cmd_dims <= S_AXI_WDATA[15:8];
                    cmd_k <= S_AXI_WDATA[23:16];
                end
                8'h08: begin
                    vec_data <= S_AXI_WDATA[15:0];
                    vec_last <= S_AXI_WDATA[31];
                end
            endcase
            b_valid <= 1'b1;
        end

        if (S_AXI_BREADY && b_valid) begin
            b_valid <= 1'b0;
        end

        // Read handling
        if (S_AXI_ARVALID && ar_ready) begin
            case (S_AXI_ARADDR)
                8'h04: r_data <= {16'b0, vector_count, 7'b0, cmd_ready};
                8'h0C: r_data <= {result_last, 7'b0, result_dist, result_idx};
                default: r_data <= 32'b0;
            endcase
            r_valid <= 1'b1;
        end

        if (S_AXI_RREADY && r_valid) begin
            r_valid <= 1'b0;
        end
    end
end

endmodule
