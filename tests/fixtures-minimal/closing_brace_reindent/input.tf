resource "aws_security_group" "x" {
  name = "sg-x"

  ingress {
    from_port = 0
    to_port   = 0
	}

  egress {
    from_port = 443
    to_port   = 443
    # allow all egress

  }

  nested {
    w = 1

}
}

   # trailing file comment
