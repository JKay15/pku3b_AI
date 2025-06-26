from pydantic import BaseModel

class search_announcements_by_title_Input(BaseModel):
    course: str
    query: str
