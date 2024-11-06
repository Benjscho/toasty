//! Postgres driver for toasty.
//!
//! Steps for getting this working:
//! 1. Get a working local postgres instance
//! 2. Create a connection against it in the example. Verify that can run
//!     sql.
//! 3. Test out the statements in the example with actual transactions.
//! 4. Validate the results are the same.

use anyhow::Result;
use sqlx::{types::Text, Any, Encode, Row, Type};
use toasty_core::{
    driver::{Capability, Operation},
    schema, sql,
    stmt::{self},
    Driver, Schema,
};

#[derive(Debug)]
pub struct Sqlx {
    pool: sqlx::Pool<Any>,
}

impl Sqlx {
    pub fn new(pool: sqlx::Pool<Any>) -> Self {
        Self { pool }
    }

    async fn create_table(&self, schema: &Schema, table: &schema::Table) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let mut params = vec![];
        let stmt: String = sql::Statement::create_table(table).to_sql_string(schema, &mut params);

        sqlx::query(&stmt).execute(&mut *tx).await?;

        unimplemented!()
    }
}

#[toasty_core::async_trait]
impl Driver for Sqlx {
    fn capability(&self) -> &Capability {
        &Capability::Sql
    }

    async fn register_schema(&mut self, _schema: &Schema) -> Result<()> {
        Ok(())
    }

    async fn exec<'stmt>(
        &self,
        schema: &Schema,
        op: Operation<'stmt>,
    ) -> Result<stmt::ValueStream<'stmt>> {
        use Operation::*;

        let sql;
        let ty;

        match &op {
            QuerySql(op) => {
                sql = &op.stmt;
                ty = op.ty.as_ref();
            }
            Insert(op) => {
                sql = op;
                ty = None;
            }
            _ => todo!(),
        }

        let mut params = vec![];
        let sql_str = sql.to_sql_string(schema, &mut params);

        if ty.is_none() {
            let exec = match sql {
                sql::Statement::Update(u) if u.pre_condition.is_some() => false,
                _ => true,
            };

            if exec {
                let mut query = sqlx::query(&sql_str);
                for param in params.iter() {
                    query = query.bind(encode_param(param));
                }
                query.execute(&self.pool).await?;

                return Ok(stmt::ValueStream::new());
            }
        }

        let mut query = sqlx::query(&sql_str);
        for param in params.iter() {
            query = query.bind(encode_param(param));
        }
        let mut tx = self.pool.begin().await?;
        let rows = query.fetch_all(&mut *tx).await?;

        let ty = match ty {
            Some(ty) => ty,
            None => &stmt::Type::Bool,
        };

        let mut ret = vec![];

        for row in rows {
            if let stmt::Type::Record(tys) = ty {
                let mut items = vec![];

                for (index, ty) in tys.iter().enumerate() {
                    items.push(load(&row, index, ty)?);
                }

                ret.push(stmt::Record::from_vec(items).into());
            } else if let stmt::Type::Bool = ty {
                ret.push(stmt::Record::from_vec(vec![]).into());
            } else {
                todo!()
            }
        }

        // Some special handling
        if let sql::Statement::Update(update) = sql {
            if update.pre_condition.is_some() && ret.is_empty() {
                // Just assume the precondition failed here... we will
                // need to make this transactional later.
                anyhow::bail!("pre condition failed");
            } else if update.returning.is_none() {
                return Ok(stmt::ValueStream::new());
            }
        }

        Ok(stmt::ValueStream::from_vec(ret))
    }

    async fn reset_db(&self, schema: &Schema) -> Result<()> {
        for table in schema.tables() {
            self.create_table(schema, table).await?;
        }

        Ok(())
    }
}

// Helper function to convert params to sqlx-compatible values
fn encode_param<'q>(value: &'q stmt::Value) -> impl Encode<'q, Any> + Type<Any> {
    use stmt::Value::*;

    match value {
        Id(v) => Text(v.to_string()),
        I64(v) => todo!(),
        String(v) => todo!(),
        Null => todo!(),
        Enum(v) => todo!(),
        _ => todo!("value = {:#?}", value),
    }
}

// Helper function to load values from a row
fn load<'a>(row: &impl Row, index: usize, ty: &stmt::Type) -> Result<stmt::Value<'a>> {
    unimplemented!()
    // match ty {
    //     stmt::Type::String => Ok(stmt::Value::String(row.try_get(index)?)),
    //     stmt::Type::Int => Ok(stmt::Value::Int(row.try_get(index)?)),
    //     stmt::Type::Float => Ok(stmt::Value::Float(row.try_get(index)?)),
    //     stmt::Type::Bool => Ok(stmt::Value::Bool(row.try_get(index)?)),
    //     stmt::Type::Bytes => Ok(stmt::Value::Bytes(row.try_get(index)?)),
    //     stmt::Type::Null => Ok(stmt::Value::Null),
    //     stmt::Type::Record(_) => {
    //         // For records, you might need to implement custom logic
    //         // depending on how you want to handle nested structures
    //         todo!("Implement record type handling")
    //     }
    //     // Add more cases as needed for your specific Type enum
    //     _ => Err(anyhow::anyhow!("Unsupported type: {:?}", ty)),
    // }
}

// #[toasty_core::async_trait()]
// impl Driver for Pgsql {
//     fn capability(&self) -> &Capability {
//         &Capability::Sql
//     }
//
//     async fn register_schema(&mut self, _schema: &Schema) -> Result<()> {
//         Ok(())
//     }
//
//     async fn exec<'a>(
//         &self,
//         schema: &'a Schema,
//         op: Operation<'a>,
//     ) -> Result<stmt::ValueStream<'a>> {
//         use Operation::*;
//
//         let connection = self.connection.lock().unwrap();
//
//         let sql;
//         let ty;
//
//         match &op {
//             QuerySql(op) => {
//                 sql = &op.stmt;
//                 ty = op.ty.as_ref();
//             }
//             Insert(op) => {
//                 sql = op;
//                 ty = None;
//             }
//             _ => todo!(),
//         }
//
//         let mut params = vec![];
//         let sql_str = sql.to_sql_string(schema, &mut params);
//
//         let mut tx = connection.begin().await?;
//         let mut stmt = tx.prepare(&sql_str);
//
//         if ty.is_none() {
//             let exec = match sql {
//                 sql::Statement::Update(u) if u.pre_condition.is_some() => false,
//                 _ => true,
//             };
//
//             if exec {
//                 let ret = stmt
//                     .execute(rusqlite::params_from_iter(
//                         params.iter().map(value_from_param),
//                     ))
//                     .unwrap();
//
//                 return Ok(stmt::ValueStream::new());
//             }
//         }
//
//         let mut rows = stmt
//             .query(rusqlite::params_from_iter(
//                 params.iter().map(value_from_param),
//             ))
//             .unwrap();
//
//         let ty = match ty {
//             Some(ty) => ty,
//             None => &stmt::Type::Bool,
//         };
//
//         let mut ret = vec![];
//
//         loop {
//             match rows.next() {
//                 Ok(Some(row)) => {
//                     if let stmt::Type::Record(tys) = ty {
//                         let mut items = vec![];
//
//                         for (index, ty) in tys.iter().enumerate() {
//                             items.push(load(row, index, ty));
//                         }
//
//                         ret.push(stmt::Record::from_vec(items).into());
//                     } else if let stmt::Type::Bool = ty {
//                         ret.push(stmt::Record::from_vec(vec![]).into());
//                     } else {
//                         todo!()
//                     }
//                 }
//                 Ok(None) => break,
//                 Err(err) => {
//                     return Err(err.into());
//                 }
//             }
//         }
//
//         // Some special handling
//         if let sql::Statement::Update(update) = sql {
//             if update.pre_condition.is_some() && ret.is_empty() {
//                 // Just assume the precondition failed here... we will
//                 // need to make this transactional later.
//                 anyhow::bail!("pre condition failed");
//             } else if update.returning.is_none() {
//                 return Ok(stmt::ValueStream::new());
//             }
//         }
//
//         Ok(stmt::ValueStream::from_vec(ret))
//     }
//
//     async fn reset_db(&self, schema: &Schema) -> Result<()> {
//         for table in &schema.tables {
//             self.create_table(schema, table);
//         }
//
//         Ok(())
//     }
// }
