module "vpc" {
  source  = "terraform-aws-modules/vpc/aws"
  version = "5.0.0"

  providers = {
    aws = aws.primary
  }

  cidr_block = "10.0.0.0/16"
  name       = "my-vpc"
}
