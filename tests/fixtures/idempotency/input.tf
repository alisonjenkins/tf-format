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
  type        = string
  description = "The AMI to use"
}

variable "region" {
  type    = string
  default = "us-east-1"
}
