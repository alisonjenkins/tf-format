resource "aws_instance" "example" {
  ami = "ami-12345678"
  instance_type = "t2.micro"
  subnet_id = "subnet-abc123"
  tags = {
    Name = "example"
    Environment = "dev"
    CostCenter = "12345"
  }
}
