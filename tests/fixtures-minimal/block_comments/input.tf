resource "aws_instance" "web" {
  /* pick a sane
     default here */
  instance_type = "t2.micro"
  ami           = "ami-123"

  tags = {
    /* the project
       this belongs to */
    Project = "demo"
    Name    = "web"
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
