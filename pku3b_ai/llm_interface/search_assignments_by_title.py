from pydantic import BaseModel

class search_assignments_by_title_Input(BaseModel):
    course: str
    query: str
