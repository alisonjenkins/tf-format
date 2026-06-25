run "apply_and_check" {
  assert {
    error_message = "bucket name mismatch"
    condition     = aws_s3_bucket.this.bucket == var.name
  }
  command = apply
  variables {
    name   = "my-bucket"
    region = "us-east-1"
  }
}

mock_provider "aws" {
  alias = "fake"
}
