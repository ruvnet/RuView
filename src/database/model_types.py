"""
Database type compatibility helpers for WiFi-DensePose API
"""

from typing import Type, Any
from sqlalchemy import String, Text, JSON
from sqlalchemy.dialects.postgresql import ARRAY as PostgreSQL_ARRAY
from sqlalchemy.ext.compiler import compiles
from sqlalchemy.sql import sqltypes


class ArrayType(sqltypes.TypeDecorator):
    """Array type that works with both PostgreSQL and SQLite."""
    
    impl = Text
    cache_ok = True
    
    def __init__(self, item_type: Type = String):
        super().__init__()
        self.item_type = item_type
    
    def load_dialect_impl(self, dialect):
        """Load dialect-specific implementation."""
        if dialect.name == 'postgresql':
            return dialect.type_descriptor(PostgreSQL_ARRAY(self.item_type))
        else:
            # For SQLite and others, use JSON
            return dialect.type_descriptor(JSON)
    
    def process_bind_param(self, value, dialect):
        """Process value before saving to database."""
        if value is None:
            return value
        
        if dialect.name == 'postgresql':
            return value
        else:
            # For SQLite, convert to JSON
            return value if isinstance(value, (list, type(None))) else list(value)
    
    def process_result_value(self, value, dialect):
        """Process value after loading from database."""
        if value is None:
            return value
        
        if dialect.name == 'postgresql':
            return value
        else:
            # For SQLite, value is already a list from JSON
            return value if isinstance(value, list) else []


def get_array_type(item_type: Type = String) -> Type:
    """Get appropriate array type based on database."""
    return ArrayType(item_type)


# Convenience types
StringArray = ArrayType(String)
FloatArray = ArrayType(sqltypes.Float)