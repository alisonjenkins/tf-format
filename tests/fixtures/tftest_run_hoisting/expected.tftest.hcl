run "apply_and_check" {
  command = apply

  variables {
    name   = "my-bucket"
    region = "us-east-1"
  }

  module {
    source = "./modules/bucket"
  }

  assert {
    condition     = aws_s3_bucket.this.bucket == var.name
    error_message = "bucket name mismatch"
  }

  expect_failures = [
    aws_s3_bucket.bad,
  ]
}
