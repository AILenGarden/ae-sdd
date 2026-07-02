-- Supabase Storage Schema Initialization
-- Creates file storage related tables in the 'storage' schema
-- Based on Supabase Storage system

-- Create storage.buckets table
CREATE TABLE IF NOT EXISTS storage.buckets
(
    id                  VARCHAR(255) PRIMARY KEY,
    name                VARCHAR(255) UNIQUE NOT NULL,
    owner               UUID REFERENCES auth.users (id),

    -- Bucket configuration
    public              BOOLEAN                  DEFAULT FALSE,
    avif_autodetection  BOOLEAN                  DEFAULT FALSE,
    file_size_limit     BIGINT,
    allowed_mime_types  TEXT[],

    -- Timestamps
    created_at          TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at          TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create trigger function to automatically update updated_at
CREATE OR REPLACE FUNCTION storage.update_updated_at_column()
    RETURNS TRIGGER AS
$$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create trigger for buckets
CREATE TRIGGER update_buckets_updated_at
    BEFORE UPDATE
    ON storage.buckets
    FOR EACH ROW
EXECUTE FUNCTION storage.update_updated_at_column();

-- Add comments
COMMENT ON TABLE storage.buckets IS 'Storage buckets for organizing files';
COMMENT ON COLUMN storage.buckets.id IS 'Unique bucket identifier';
COMMENT ON COLUMN storage.buckets.name IS 'Bucket name (must be unique)';
COMMENT ON COLUMN storage.buckets.owner IS 'User ID of bucket owner';
COMMENT ON COLUMN storage.buckets.public IS 'Whether bucket contents are publicly accessible';
COMMENT ON COLUMN storage.buckets.avif_autodetection IS 'Enable AVIF format auto-detection';
COMMENT ON COLUMN storage.buckets.file_size_limit IS 'Maximum file size in bytes (null = no limit)';
COMMENT ON COLUMN storage.buckets.allowed_mime_types IS 'Array of allowed MIME types (null = all types allowed)';

-- Enable Row Level Security
ALTER TABLE storage.buckets ENABLE ROW LEVEL SECURITY;

-- Create storage.objects table
CREATE TABLE IF NOT EXISTS storage.objects
(
    id               UUID PRIMARY KEY         DEFAULT gen_random_uuid(),
    bucket_id        VARCHAR(255) REFERENCES storage.buckets (id) ON DELETE CASCADE,
    name             TEXT NOT NULL,
    owner            UUID REFERENCES auth.users (id),

    -- File metadata
    version          VARCHAR(100),
    metadata         JSONB                    DEFAULT CAST('{}' AS JSONB),
    user_metadata    JSONB                    DEFAULT CAST('{}' AS JSONB),
    path_tokens      TEXT[] GENERATED ALWAYS AS (string_to_array(name, '/')) STORED,

    -- Timestamps
    created_at       TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at       TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_accessed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CONSTRAINT objects_bucket_id_name_unique UNIQUE (bucket_id, name)
);

-- Create trigger for objects
CREATE TRIGGER update_objects_updated_at
    BEFORE UPDATE
    ON storage.objects
    FOR EACH ROW
EXECUTE FUNCTION storage.update_updated_at_column();

-- Add comments
COMMENT ON TABLE storage.objects IS 'Stored file objects';
COMMENT ON COLUMN storage.objects.id IS 'Unique object identifier';
COMMENT ON COLUMN storage.objects.bucket_id IS 'Bucket that contains this object';
COMMENT ON COLUMN storage.objects.name IS 'Object name/path within the bucket';
COMMENT ON COLUMN storage.objects.owner IS 'User ID of object owner';
COMMENT ON COLUMN storage.objects.version IS 'Object version identifier';
COMMENT ON COLUMN storage.objects.metadata IS 'Custom metadata as JSON';
COMMENT ON COLUMN storage.objects.path_tokens IS 'Tokenized path for hierarchical queries';

-- Enable Row Level Security
ALTER TABLE storage.objects ENABLE ROW LEVEL SECURITY;

-- Create indexes for buckets table
CREATE INDEX IF NOT EXISTS buckets_owner_idx ON storage.buckets(owner) WHERE owner IS NOT NULL;

-- Create indexes for objects table
CREATE INDEX IF NOT EXISTS objects_bucket_id_idx ON storage.objects(bucket_id);
CREATE INDEX IF NOT EXISTS objects_name_idx ON storage.objects(name);
CREATE INDEX IF NOT EXISTS objects_owner_idx ON storage.objects(owner) WHERE owner IS NOT NULL;
CREATE INDEX IF NOT EXISTS objects_path_tokens_idx ON storage.objects USING GIN (path_tokens);

-- Performance optimization comments
COMMENT ON INDEX storage.objects_bucket_id_idx IS 'Fast lookup of objects by bucket';
COMMENT ON INDEX storage.objects_name_idx IS 'Fast lookup of objects by name';
COMMENT ON INDEX storage.objects_path_tokens_idx IS 'Fast hierarchical path queries using GIN index';

-- ============================================================
-- Vector Storage Tables
-- Aligned with Supabase Vector Bucket implementation (migrations 0044 + 0045)
-- ============================================================

-- Create vector buckets table
CREATE TABLE IF NOT EXISTS storage.buckets_vectors
(
    id         TEXT PRIMARY KEY,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Create vector indexes table
CREATE TABLE IF NOT EXISTS storage.vector_indexes
(
    id                     TEXT PRIMARY KEY         DEFAULT gen_random_uuid(),
    name                   TEXT COLLATE "C"         NOT NULL,
    bucket_id              TEXT                     NOT NULL REFERENCES storage.buckets_vectors (id),
    data_type              TEXT                     NOT NULL,
    dimension              INTEGER                  NOT NULL,
    distance_metric        TEXT                     NOT NULL,
    metadata_configuration JSONB                    NULL,
    created_at             TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at             TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Unique constraint: index name must be unique within a bucket
CREATE UNIQUE INDEX IF NOT EXISTS vector_indexes_name_bucket_id_idx
    ON storage.vector_indexes (name, bucket_id);

-- Enable Row Level Security
ALTER TABLE storage.buckets_vectors ENABLE ROW LEVEL SECURITY;
ALTER TABLE storage.vector_indexes ENABLE ROW LEVEL SECURITY;

-- Create triggers for auto-updating updated_at
CREATE TRIGGER update_buckets_vectors_updated_at
    BEFORE UPDATE
    ON storage.buckets_vectors
    FOR EACH ROW
EXECUTE FUNCTION storage.update_updated_at_column();

CREATE TRIGGER update_vector_indexes_updated_at
    BEFORE UPDATE
    ON storage.vector_indexes
    FOR EACH ROW
EXECUTE FUNCTION storage.update_updated_at_column();

-- Comments
COMMENT ON TABLE storage.buckets_vectors IS 'Vector storage buckets for organizing vector indexes';
COMMENT ON TABLE storage.vector_indexes IS 'Vector indexes within buckets, defining dimension and distance metric';
