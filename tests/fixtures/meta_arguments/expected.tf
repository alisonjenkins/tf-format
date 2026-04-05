resource "aws_instance" "example" {
  count = 3

  lifecycle {
    create_before_destroy = true
  }

  ami           = "ami-12345678"
  instance_type = "t2.micro"
  subnet_id     = "subnet-abc123"
  tags          = { Name = "example" }
}

module "vpc" {
  source  = "terraform-aws-modules/vpc/aws"
  version = "5.0.0"

  cidr_block = "10.0.0.0/16"
  name       = "my-vpc"
}
