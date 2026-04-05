resource "aws_instance" "example" {
  ami           = "ami-12345678"
  instance_type = "t2.micro"
  count         = 3
  tags          = { Name = "example" }
  lifecycle {
    create_before_destroy = true
  }
  subnet_id = "subnet-abc123"
}

module "vpc" {
  cidr_block = "10.0.0.0/16"
  source     = "terraform-aws-modules/vpc/aws"
  version    = "5.0.0"
  name       = "my-vpc"
}
