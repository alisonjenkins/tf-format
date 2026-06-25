run "apply_and_check" {
  assert {
    error_message = "bucket name mismatch"
    condition     = aws_s3_bucket.this.bucket == var.name
  }
  command = apply
  expect_failures = [
    aws_s3_bucket.bad,
  ]
  variables {
    name   = "my-bucket"
    region = "us-east-1"
  }
  module {
    source = "./modules/bucket"
  }
}
