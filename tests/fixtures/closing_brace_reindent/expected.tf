resource "aws_security_group" "x" {
  name = "sg-x"

  egress {
    from_port = 443
    to_port   = 443
    # allow all egress
  }

  ingress {
    from_port = 0
    to_port   = 0
  }
}
