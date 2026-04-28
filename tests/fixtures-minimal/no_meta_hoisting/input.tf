resource "aws_instance" "x" {
  ami           = "ami-123"
  instance_type = "t3.micro"
  count         = var.count
}
