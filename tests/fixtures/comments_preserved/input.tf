resource "aws_instance" "example" {
  # The instance type to use
  instance_type = "t2.micro"
  # The AMI to launch
  ami           = "ami-12345678"
  subnet_id     = "subnet-abc123"
}
