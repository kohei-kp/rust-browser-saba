use crate::renderer::dom::api::get_element_by_id;
use crate::renderer::dom::node::Node as DomNode;
use crate::renderer::dom::node::NodeKind as DomNodeKind;
use crate::renderer::js::ast::Node;
use crate::renderer::js::ast::Program;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::borrow::Borrow;
use core::cell::RefCell;
use core::fmt::Display;
use core::fmt::Formatter;
use core::ops::Add;
use core::ops::Sub;

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeValue {
    /// https://262.ecma-international.org/#sec-numeric-types
    Number(u64),
    StringLiteral(String),
    HtmlElement {
        object: Rc<RefCell<DomNode>>,
        property: Option<String>,
    },
}

impl Display for RuntimeValue {
    fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
        let s = match self {
            RuntimeValue::Number(value) => format!("{}", value),
            RuntimeValue::StringLiteral(value) => value.to_string(),
            RuntimeValue::HtmlElement {
                object,
                property: _,
            } => {
                format!("HtmlElement {:#?}", object)
            }
        };
        write!(f, "{}", s)
    }
}

impl Add<RuntimeValue> for RuntimeValue {
    type Output = RuntimeValue;

    fn add(self, rhs: RuntimeValue) -> RuntimeValue {
        if let (RuntimeValue::Number(left_num), RuntimeValue::Number(right_num)) = (&self, &rhs) {
            return RuntimeValue::Number(left_num + right_num);
        }
        RuntimeValue::StringLiteral(self.to_string() + &rhs.to_string())
    }
}

impl Sub<RuntimeValue> for RuntimeValue {
    type Output = RuntimeValue;

    fn sub(self, other: Self) -> RuntimeValue {
        if let (RuntimeValue::Number(left_num), RuntimeValue::Number(right_num)) = (&self, &other) {
            return RuntimeValue::Number(left_num - right_num);
        }

        // NaN: Not aNumber
        RuntimeValue::Number(u64::MIN)
    }
}

type VariableMap = Vec<(String, Option<RuntimeValue>)>;

/// https:://262.ecma-international.org/#sec-environment-records
#[derive(Debug, Clone)]
pub struct Environment {
    variables: VariableMap,
    outer: Option<Rc<RefCell<Environment>>>,
}

impl Environment {
    fn new(outer: Option<Rc<RefCell<Environment>>>) -> Self {
        Self {
            variables: VariableMap::new(),
            outer,
        }
    }

    pub fn get_variable(&self, name: String) -> Option<RuntimeValue> {
        for variable in &self.variables {
            if variable.0 == name {
                return variable.1.clone();
            }
        }
        if let Some(env) = &self.outer {
            env.borrow_mut().get_variable(name)
        } else {
            None
        }
    }

    fn add_variable(&mut self, name: String, value: Option<RuntimeValue>) {
        self.variables.push((name, value));
    }

    fn update_variable(&mut self, name: String, value: Option<RuntimeValue>) {
        for i in 0..self.variables.len() {
            // もし変数を見つけた場合、今までの名前と値のタプルを削除し、新しい値とのタプルを追加する
            if self.variables[i].0 == name {
                self.variables.remove(i);
                self.variables.push((name, value));
                return;
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    id: String,
    params: Vec<Option<Rc<Node>>>,
    body: Option<Rc<Node>>,
}

impl Function {
    fn new(id: String, params: Vec<Option<Rc<Node>>>, body: Option<Rc<Node>>) -> Self {
        Self { id, params, body }
    }
}

#[derive(Debug, Clone)]
pub struct JsRuntime {
    dom_root: Rc<RefCell<DomNode>>,
    env: Rc<RefCell<Environment>>,
    functions: Vec<Function>,
}

impl JsRuntime {
    pub fn new(dom_root: Rc<RefCell<DomNode>>) -> Self {
        Self {
            dom_root,
            functions: Vec::new(),
            env: Rc::new(RefCell::new(Environment::new(None))),
        }
    }

    pub fn execute(&mut self, program: &Program) {
        for node in program.body() {
            self.eval(&Some(node.clone()), self.env.clone());
        }
    }

    fn eval(
        &mut self,
        node: &Option<Rc<Node>>,
        env: Rc<RefCell<Environment>>,
    ) -> Option<RuntimeValue> {
        let node = match node {
            Some(n) => n,
            None => return None,
        };

        match node.borrow() {
            Node::ExpressionStatement(expr) => return self.eval(&expr, env.clone()),
            Node::AdditiveExpression {
                operator,
                left,
                right,
            } => {
                let left_value = match self.eval(&left, env.clone()) {
                    Some(value) => value,
                    None => return None,
                };
                let right_value = match self.eval(&right, env.clone()) {
                    Some(value) => value,
                    None => return None,
                };

                if operator == &'+' {
                    Some(left_value + right_value)
                } else if operator == &'-' {
                    Some(left_value - right_value)
                } else {
                    None
                }
            }
            Node::AssignmentExpression {
                operator,
                left,
                right,
            } => {
                if operator != &'=' {
                    return None;
                }
                // 変数の再割り当て
                if let Some(node) = left {
                    if let Node::Identifier(id) = node.borrow() {
                        let new_value = self.eval(right, env.clone());
                        env.borrow_mut().update_variable(id.to_string(), new_value);
                        return None;
                    }
                }

                // もし左辺の値がDOMツリーのノードを表すHtmlElementならば、DOMツリーを更新する
                if let Some(RuntimeValue::HtmlElement { object, property }) =
                    self.eval(left, env.clone())
                {
                    let right_value = match self.eval(right, env.clone()) {
                        Some(value) => value,
                        None => return None,
                    };

                    if let Some(p) = property {
                        // target.textContent = "foobar";のようにノードのテキストを変更する
                        if p == "textContent" {
                            object
                                .borrow_mut()
                                .set_first_child(Some(Rc::new(RefCell::new(DomNode::new(
                                    DomNodeKind::Text(right_value.to_string()),
                                )))));
                        }
                    }
                }
                None
            }
            Node::MemberExpression { object, property } => {
                let object_value = match self.eval(object, env.clone()) {
                    Some(value) => value,
                    None => return None,
                };
                let property_value = match self.eval(property, env.clone()) {
                    Some(value) => value,
                    // プロパティが存在しないため、object_valueをここで返す
                    None => return Some(object_value),
                };
                // もしオブジェクトがDOMノードの場合、HtmlElementのpropertyを更新する
                if let RuntimeValue::HtmlElement { object, property } = object_value {
                    assert!(property.is_none());
                    // HtmlElementのpropertyにproperty_valueの文字列をセットする
                    return Some(RuntimeValue::HtmlElement {
                        object,
                        property: Some(property_value.to_string()),
                    });
                }

                // document.getElementByIdは、"document.getElementById"という一つの文字列として扱う
                // このメソッドへの呼び出しは、"document.getElementById"という名前の関数への呼び出しになる
                return Some(
                    object_value + RuntimeValue::StringLiteral(".".to_string()) + property_value,
                );
            }
            Node::NumericLiteral(value) => Some(RuntimeValue::Number(*value)),
            Node::VariableDeclaration { declarations } => {
                for declaration in declarations {
                    self.eval(&declaration, env.clone());
                }
                None
            }
            Node::VariableDeclarator { id, init } => {
                if let Some(node) = id {
                    if let Node::Identifier(id) = node.borrow() {
                        let init = self.eval(&init, env.clone());
                        env.borrow_mut().add_variable(id.to_string(), init);
                    }
                }
                None
            }
            Node::Identifier(name) => {
                match env.borrow_mut().get_variable(name.to_string()) {
                    Some(v) => Some(v),
                    // 変数名が初めて使用される場合は、まだ値は保存されていないので、文字列として扱う
                    // たとえば、var a = 42;のようなコードの場合、aはStringLiteralとして扱われる
                    None => Some(RuntimeValue::StringLiteral(name.to_string())),
                }
            }
            Node::StringLiteral(value) => Some(RuntimeValue::StringLiteral(value.to_string())),
            Node::CallExpression { callee, arguments } => {
                // 新しいスコープを作成する
                let new_env = Rc::new(RefCell::new(Environment::new(Some(env))));

                let callee_value = match self.eval(callee, new_env.clone()) {
                    Some(value) => value,
                    None => return None,
                };

                // ブラウザAPIを呼び出す
                let api_result = self.call_browser_api(&callee_value, arguments, new_env.clone());
                if api_result.0 {
                    return api_result.1;
                }

                let function = {
                    let mut f: Option<Function> = None;

                    for func in &self.functions {
                        if callee_value == RuntimeValue::StringLiteral(func.id.to_string()) {
                            f = Some(func.clone());
                        }
                    }

                    match f {
                        Some(f) => f,
                        None => panic!("function {:?} doesn't exist", callee),
                    }
                };

                // 関数呼び出し時に渡される引数を新しく作成したスコープのローカル変数として割り当てる
                assert!(arguments.len() == function.params.len());
                for (i, item) in arguments.iter().enumerate() {
                    if let Some(RuntimeValue::StringLiteral(name)) =
                        self.eval(&function.params[i], new_env.clone())
                    {
                        new_env
                            .borrow_mut()
                            .add_variable(name, self.eval(item, new_env.clone()));
                    }
                }

                // 関数を新しいスコープとともに呼ぶ
                self.eval(&function.body.clone(), new_env.clone())
            }
            Node::BlockStatement { body } => {
                let mut result: Option<RuntimeValue> = None;
                for stmt in body {
                    result = self.eval(&stmt, env.clone());
                }
                result
            }
            Node::ReturnStatement { argument } => {
                return self.eval(&argument, env.clone());
            }
            Node::FunctionDeclaration { id, params, body } => {
                if let Some(RuntimeValue::StringLiteral(id)) = self.eval(&id, env.clone()) {
                    let cloned_body = match body {
                        Some(b) => Some(b.clone()),
                        None => None,
                    };
                    self.functions
                        .push(Function::new(id, params.to_vec(), cloned_body));
                };
                None
            }
        }
    }

    /// (bool, Option<RuntimeValue>)のタプルを返す
    /// bool: ブラウザAPIが呼ばれたかどうか。trueなら何かしらのAPIが呼ばれたことを示す
    /// Option<RuntimeValue>: ブラウザAPIの呼び出しによって得られた結果
    fn call_browser_api(
        &mut self,
        func: &RuntimeValue,
        arguments: &[Option<Rc<Node>>],
        env: Rc<RefCell<Environment>>,
    ) -> (bool, Option<RuntimeValue>) {
        if func == &RuntimeValue::StringLiteral("document.getElementById".to_string()) {
            let arg = match self.eval(&arguments[0], env.clone()) {
                Some(a) => a,
                None => return (true, None),
            };
            let target = match get_element_by_id(Some(self.dom_root.clone()), &arg.to_string()) {
                Some(t) => t,
                None => return (true, None),
            };
            return (
                true,
                Some(RuntimeValue::HtmlElement {
                    object: target,
                    property: None,
                }),
            );
        }

        (false, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::js::ast::JsParser;
    use crate::renderer::js::token::JsLexer;

    #[test]
    fn test_num() {
        let input = "42".to_string();
        let lexer = JsLexer::new(input);
        let mut parser = JsParser::new(lexer);
        let ast = parser.parse_ast();
        let dom = Rc::new(RefCell::new(DomNode::new(DomNodeKind::Document)));
        let mut runtime = JsRuntime::new(dom);
        let expected = [Some(RuntimeValue::Number(42))];
        let mut i = 0;

        for node in ast.body() {
            let result = runtime.eval(&Some(node.clone()), runtime.env.clone());
            assert_eq!(expected[i], result);
            i += 1;
        }
    }

    #[test]
    fn test_add_nums() {
        let input = "1 + 2".to_string();
        let lexer = JsLexer::new(input);
        let mut parser = JsParser::new(lexer);
        let ast = parser.parse_ast();
        let dom = Rc::new(RefCell::new(DomNode::new(DomNodeKind::Document)));
        let mut runtime = JsRuntime::new(dom);
        let expected = [Some(RuntimeValue::Number(3))];
        let mut i = 0;

        for node in ast.body() {
            let result = runtime.eval(&Some(node.clone()), runtime.env.clone());
            assert_eq!(expected[i], result);
            i += 1;
        }
    }

    #[test]
    fn test_sub_nums() {
        let input = "2 - 1".to_string();
        let lexer = JsLexer::new(input);
        let mut parser = JsParser::new(lexer);
        let ast = parser.parse_ast();
        let dom = Rc::new(RefCell::new(DomNode::new(DomNodeKind::Document)));
        let mut runtime = JsRuntime::new(dom);
        let expected = [Some(RuntimeValue::Number(1))];
        let mut i = 0;

        for node in ast.body() {
            let result = runtime.eval(&Some(node.clone()), runtime.env.clone());
            assert_eq!(expected[i], result);
            i += 1;
        }
    }

    #[test]
    fn test_assign_variable() {
        let input = "var foo=42;".to_string();
        let lexer = JsLexer::new(input);
        let mut parser = JsParser::new(lexer);
        let ast = parser.parse_ast();
        let dom = Rc::new(RefCell::new(DomNode::new(DomNodeKind::Document)));
        let mut runtime = JsRuntime::new(dom);
        let expected = [None];
        let mut i = 0;

        for node in ast.body() {
            let result = runtime.eval(&Some(node.clone()), runtime.env.clone());
            assert_eq!(expected[i], result);
            i += 1;
        }
    }

    #[test]
    fn test_add_variable_and_num() {
        let input = "var foo=42; foo+1;".to_string();
        let lexer = JsLexer::new(input);
        let mut parser = JsParser::new(lexer);
        let ast = parser.parse_ast();
        let dom = Rc::new(RefCell::new(DomNode::new(DomNodeKind::Document)));
        let mut runtime = JsRuntime::new(dom);
        let expected = [None, Some(RuntimeValue::Number(43))];
        let mut i = 0;

        for node in ast.body() {
            let result = runtime.eval(&Some(node.clone()), runtime.env.clone());
            assert_eq!(expected[i], result);
            i += 1;
        }
    }

    #[test]
    fn test_reassign_variable() {
        let input = "var foo=42; foo=1; foo".to_string();
        let lexer = JsLexer::new(input);
        let mut parser = JsParser::new(lexer);
        let ast = parser.parse_ast();
        let dom = Rc::new(RefCell::new(DomNode::new(DomNodeKind::Document)));
        let mut runtime = JsRuntime::new(dom);
        let expected = [None, None, Some(RuntimeValue::Number(1))];
        let mut i = 0;

        for node in ast.body() {
            let result = runtime.eval(&Some(node.clone()), runtime.env.clone());
            assert_eq!(expected[i], result);
            i += 1;
        }
    }
}
