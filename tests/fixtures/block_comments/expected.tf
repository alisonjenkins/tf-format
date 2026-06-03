resource "aws_instance" "web" {
  ami = "ami-123"
  /* pick a sane
     default here */
  instance_type = "t2.micro"

  tags = {
    Name = "web"
    /* the project
       this belongs to */
    Project = "demo"
  }
}

# standalone bucket follows
resource "aws_s3_bucket" "data" {
  /*
   * star-aligned
   * block comment
   */
  bucket = "my-bucket"
}
