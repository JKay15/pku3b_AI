from pydantic import BaseModel

class search_entry_by_title_Input(BaseModel):
    course: str
    query: str
