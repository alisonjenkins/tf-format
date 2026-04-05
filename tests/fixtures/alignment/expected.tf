resource "aws_instance" "example" {
  ami           = "ami-12345678"
  instance_type = "t2.micro"
  subnet_id     = "subnet-abc123"

  tags = {
    CostCenter  = "12345"
    Environment = "dev"
    Name        = "example"
  }
}
