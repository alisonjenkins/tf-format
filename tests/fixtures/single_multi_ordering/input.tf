resource "aws_instance" "example" {
  tags = {
    Name        = "example"
    Environment = "dev"
  }
  ami           = "ami-12345678"
  lifecycle {
    create_before_destroy = true
  }
  instance_type = "t2.micro"
  ebs_block_device {
    volume_size = 20
    device_name = "/dev/sda1"
  }
  subnet_id     = "subnet-abc123"
}
