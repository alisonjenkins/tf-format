mock_provider "aws" {
  alias = "fake"

  mock_resource "aws_s3_bucket" {
    defaults = {
      arn = "arn:aws:s3:::mock"
    }
  }

  override_data {
    target = data.aws_caller_identity.current

    values = {
      account_id = "123456789012"
    }
  }
}

run "uses_mock" {
  command = plan

  providers = {
    aws = aws.fake
  }
}
