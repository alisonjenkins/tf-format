resource "aws_instance" "example" {
  ami           = "ami-12345678"
  instance_type = "t2.micro"
  subnet_id     = "subnet-abc123"

  tags = {
    Environment = "dev"
    Name        = "example"
  }
}

variable "ami" {
  description = "The AMI to use"
  type        = string
}

variable "region" {
  default = "us-east-1"
  type    = string
}
