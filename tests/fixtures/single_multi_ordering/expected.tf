resource "aws_instance" "example" {
  ami           = "ami-12345678"
  instance_type = "t2.micro"
  subnet_id     = "subnet-abc123"

  ebs_block_device {
    device_name = "/dev/sda1"
    volume_size = 20
  }

  lifecycle {
    create_before_destroy = true
  }

  tags = {
    Environment = "dev"
    Name        = "example"
  }
}
