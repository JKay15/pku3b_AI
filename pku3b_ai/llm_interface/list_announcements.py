from pydantic import BaseModel

class list_announcements_Input(BaseModel):
    course: int
