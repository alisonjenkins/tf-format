resource "aws_instance" "x" {
  depends_on = [aws_vpc.main]
  ami        = "ami-1"
  count      = 2
  provider   = aws.east
}
