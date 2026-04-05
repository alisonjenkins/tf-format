resource "aws_security_group" "example" {
  description = "Example security group"
  name        = "example"
  vpc_id      = "vpc-abc123"

  egress {
    cidr_blocks = ["0.0.0.0/0"]
    from_port   = 0
    protocol    = "-1"
    to_port     = 0
  }

  ingress {
    cidr_blocks = ["10.0.0.0/8"]
    from_port   = 443
    protocol    = "tcp"
    to_port     = 443
  }

  ingress {
    cidr_blocks = ["10.0.0.0/8"]
    from_port   = 80
    protocol    = "tcp"
    to_port     = 80
  }
}
