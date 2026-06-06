resource "aws_instance" "web" {
  ami           = "ami-123"
  instance_type = "t3.micro"

  tags = {
    Name = "web"
    Env  = "prod"
  }
}

variable "region" {
  type    = string
  default = "eu-west-1"
}
