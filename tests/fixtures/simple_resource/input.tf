resource "aws_instance" "example" {
  instance_type = "t2.micro"
  ami           = "ami-12345678"
  tags          = { Name = "example" }
  subnet_id     = "subnet-abc123"
}
