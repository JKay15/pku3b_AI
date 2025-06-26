from pydantic import BaseModel

class search_videos_by_title_Input(BaseModel):
    course: str
    query: str
