-- ============================================================================
-- Embedding Generation Functions
-- ============================================================================

-- Generate embedding from text using default or specified model
CREATE OR REPLACE FUNCTION ruvector_embed(text text, model_name text DEFAULT 'all-MiniLM-L6-v2')
RETURNS real[]
AS 'MODULE_PATHNAME', 'ruvector_embed_wrapper'
LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

-- Generate embeddings for multiple texts in batch
CREATE OR REPLACE FUNCTION ruvector_embed_batch(texts text[], model_name text DEFAULT 'all-MiniLM-L6-v2')
RETURNS real[][]
AS 'MODULE_PATHNAME', 'ruvector_embed_batch_wrapper'
LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

-- List all available embedding models
CREATE OR REPLACE FUNCTION ruvector_embedding_models()
RETURNS TABLE (
    model_name text,
    dimensions integer,
    description text,
    is_loaded boolean
)
AS 'MODULE_PATHNAME', 'ruvector_embedding_models_wrapper'
LANGUAGE C IMMUTABLE STRICT;

-- Load embedding model into memory
CREATE OR REPLACE FUNCTION ruvector_load_model(model_name text)
RETURNS boolean
AS 'MODULE_PATHNAME', 'ruvector_load_model_wrapper'
LANGUAGE C STRICT;

-- Unload embedding model from memory
CREATE OR REPLACE FUNCTION ruvector_unload_model(model_name text)
RETURNS boolean
AS 'MODULE_PATHNAME', 'ruvector_unload_model_wrapper'
LANGUAGE C STRICT;

-- Get information about a specific model
CREATE OR REPLACE FUNCTION ruvector_model_info(model_name text)
RETURNS jsonb
AS 'MODULE_PATHNAME', 'ruvector_model_info_wrapper'
LANGUAGE C IMMUTABLE STRICT;

-- Set default embedding model
CREATE OR REPLACE FUNCTION ruvector_set_default_model(model_name text)
RETURNS boolean
AS 'MODULE_PATHNAME', 'ruvector_set_default_model_wrapper'
LANGUAGE C STRICT;

-- Get current default embedding model
CREATE OR REPLACE FUNCTION ruvector_default_model()
RETURNS text
AS 'MODULE_PATHNAME', 'ruvector_default_model_wrapper'
LANGUAGE C IMMUTABLE STRICT;

-- Get embedding generation statistics
CREATE OR REPLACE FUNCTION ruvector_embedding_stats()
RETURNS jsonb
AS 'MODULE_PATHNAME', 'ruvector_embedding_stats_wrapper'
LANGUAGE C IMMUTABLE STRICT;

-- Get dimensions for a specific model
CREATE OR REPLACE FUNCTION ruvector_embedding_dims(model_name text)
RETURNS integer
AS 'MODULE_PATHNAME', 'ruvector_embedding_dims_wrapper'
LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;
