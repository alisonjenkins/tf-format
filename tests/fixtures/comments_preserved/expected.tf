resource "aws_instance" "example" {
  # The AMI to launch
  ami = "ami-12345678"
  # The instance type to use
  instance_type = "t2.micro"
  subnet_id     = "subnet-abc123"
}
